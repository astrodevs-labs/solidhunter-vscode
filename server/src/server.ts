/* --------------------------------------------------------------------------------------------
 * Copyright (c) Microsoft Corporation. All rights reserved.
 * Licensed under the MIT License. See License.txt in the project root for license information.
 * ------------------------------------------------------------------------------------------ */
import {
	createConnection,
	TextDocuments,
	Diagnostic,
	DiagnosticSeverity,
	ProposedFeatures,
	InitializeParams,
	DidChangeConfigurationNotification,
	TextDocumentChangeRegistrationOptions,
	CompletionItem,
	CompletionItemKind,
	TextDocumentPositionParams,
	TextDocumentSyncKind,
	InitializeResult,
	DidChangeTextDocumentNotification,
	Range
} from 'vscode-languageserver/node';

import {
	TextDocument
} from 'vscode-languageserver-textdocument';
import { exec } from 'child_process';
import { resolve } from 'path';
import * as fs from 'fs';
// Create a connection for the server, using Node's IPC as a transport.
// Also include all preview / proposed LSP features.
const connection = createConnection(ProposedFeatures.all);

const linterPath = "/home/mindblower78/Documents/astrodevs-labs/solidhunter/target/release/solidhunter";

const severity_to_value = (value: string) => {
	if (value == "WARNING") {
		return 2;
	} else if (value == "ERROR") {
		return 1;
	} else if (value == "INFO") {
		return 3;
	} else if (value == "HINT") {
		return 4;
	}
	return 1;
};


let configPath = "";
/*
{
    "range": {
      "start": {
        "line": 4,
        "character": 9
      },
      "end": {
        "line": 5,
        "character": 5
      },
      "length": 5
    },
    "severity": "WARNING",
    "message": "Contract name need to be in pascal case",
    "uri": "test.sol",
    "sourceFileContent": "//SPDX-License-Identifier: MIT\npragma solidity ^0.8.0;\n\ncontract Test_ {\n    function Test_() {\n        \n    }\n}"
  },

*/


const lint_file = async (filepath: string) : Promise<Diagnostic[]> => {
	return new Promise<Diagnostic[]>(async (resolve, reject) => {
		if (configPath === "")
		{
			const folders = await connection.workspace.getWorkspaceFolders();
			if (folders)
				configPath = folders[0].uri.replace('file://', '') + "/.solidhunter.json";
		}
		const diags : Diagnostic[] = [];
		exec(linterPath + " -j -f " +  filepath + " -r " + configPath, (err, out, err_out) => {
			if (err) {
				console.log(err);
			}
			if (out) {
				console.log("OUTPUT: \n" + out);
				let out_diags;
				if (out[0] != 'E') {
					out_diags = JSON.parse(out);
				} else {
					out_diags = undefined;
				}
			if (out_diags != undefined)
				{
					console.log("OUTPUT JSON: \n" + out_diags);
					out_diags.forEach((elem: any) => {
						const diagnostic: Diagnostic = {
							severity: severity_to_value(elem.severity),
							range: Range.create(elem.range.start.line - 1, elem.range.start.character, elem.range.end.line - 1, elem.range.end.character),					
							message: elem.message,
							source: 'solidhunter'
						};
						diags.push(diagnostic);
					});
				}
			}
			if (err_out) {
				console.log(`error with cmd : ${err_out}`);
			}
			console.log("generated : " + diags.length + " diags");
			resolve(diags);
		});
	});
	
};

// Create a simple text document manager.
const documents: TextDocuments<TextDocument> = new TextDocuments(TextDocument);

let hasConfigurationCapability = false;
let hasWorkspaceFolderCapability = false;
let hasDiagnosticRelatedInformationCapability = false;


connection.onInitialize((params: InitializeParams) => {
	if (params.workspaceFolders)
		configPath = params.workspaceFolders[0].uri.replace('file://', '') + "/.solidhunter.json";
	const capabilities = params.capabilities;

	// Does the client support the `workspace/configuration` request?
	// If not, we fall back using global settings.
	hasConfigurationCapability = !!(
		capabilities.workspace && !!capabilities.workspace.configuration
	);
	hasWorkspaceFolderCapability = !!(
		capabilities.workspace && !!capabilities.workspace.workspaceFolders
	);
	hasDiagnosticRelatedInformationCapability = !!(
		capabilities.textDocument &&
		capabilities.textDocument.publishDiagnostics &&
		capabilities.textDocument.publishDiagnostics.relatedInformation
	);

	const result: InitializeResult = {
		capabilities: {
			textDocumentSync: TextDocumentSyncKind.Full,
			// Tell the client that this server supports code completion.
			completionProvider: {
				resolveProvider: false
			}
		}
	};
	result.capabilities.workspace = {
		workspaceFolders: {
			supported: true
		}
	};
	return result;
});


connection.onInitialized(() => {
	if (hasConfigurationCapability) {
		// Register for all configuration changes.
		connection.client.register(DidChangeConfigurationNotification.type, undefined);
	}
	connection.client.register(DidChangeTextDocumentNotification.type, undefined);
	if (hasWorkspaceFolderCapability) {
		connection.workspace.onDidChangeWorkspaceFolders(_event => {
			connection.console.log('Workspace folder change event received.');
		});
	}
});

connection.onDidChangeTextDocument(_changes => {
	connection.console.error("issou");
	const doc = documents.get(_changes.textDocument.uri);
	if (doc != undefined && doc.languageId == 'sol')
		validateTextDocument(doc);
	else {
		connection.console.error("Wrong text type or not found");
	}
});

// The example settings
interface ExampleSettings {
	maxNumberOfProblems: number;
}

// The global settings, used when the `workspace/configuration` request is not supported by the client.
// Please note that this is not the case when using this server with the client provided in this example
// but could happen with other clients.
const defaultSettings: ExampleSettings = { maxNumberOfProblems: 1000 };
let globalSettings: ExampleSettings = defaultSettings;

// Cache the settings of all open documents
const documentSettings: Map<string, Thenable<ExampleSettings>> = new Map();

connection.onDidChangeConfiguration(change => {
	if (hasConfigurationCapability) {
		// Reset all cached document settings
		documentSettings.clear();
	} else {
		globalSettings = <ExampleSettings>(
			(change.settings.languageServerExample || defaultSettings)
		);
	}

	// Revalidate all open text documents
	documents.all().forEach(validateTextDocument);
});

function getDocumentSettings(resource: string): Thenable<ExampleSettings> {
	if (!hasConfigurationCapability) {
		return Promise.resolve(globalSettings);
	}
	let result = documentSettings.get(resource);
	if (!result) {
		result = connection.workspace.getConfiguration({
			scopeUri: resource,
			section: 'solidhunter'
		});
		documentSettings.set(resource, result);
	}
	return result;
}

// Only keep settings for open documents
documents.onDidClose(e => {
	documentSettings.delete(e.document.uri);
});

// The content of a text document has changed. This event is emitted
// when the text document first opened or when its content has changed.
documents.onDidChangeContent(change => {
	connection.console.log('File changed');
});

documents.onDidSave(file => {
	validateTextDocument(file.document);
});

documents.onDidOpen(file => {
	validateTextDocument(file.document);
});

async function validateTextDocument(textDocument: TextDocument): Promise<void> {
	// In this simple example we get the settings for every validate run.
	const settings = await getDocumentSettings(textDocument.uri);

	// The validator creates diagnostics for all uppercase words length 2 and more
	const text = textDocument.getText();
	const pattern = /\b[A-Z]{2,}\b/g;
	let m: RegExpExecArray | null;

	const diagnostics: Diagnostic[] = await lint_file(textDocument.uri.replace('file://', ''));

	console.log("diag nb: " + diagnostics.length);
	// Send the computed diagnostics to VSCode.
	connection.sendDiagnostics({ uri: textDocument.uri, diagnostics });
}

connection.onDidChangeWatchedFiles(_change => {
	// Monitored files have change in VSCode
	connection.console.log('We received an file change event');
});

// This handler provides the initial list of the completion items.
/*
connection.onCompletion(
	(_textDocumentPosition: TextDocumentPositionParams): CompletionItem[] => {
		// The pass parameter contains the position of the text document in
		// which code complete got requested. For the example we ignore this
		// info and always provide the same completion items.
		return [
			{
				label: 'TypeScript',
				kind: CompletionItemKind.Text,
				data: 1
			},
			{
				label: 'JavaScript',
				kind: CompletionItemKind.Text,
				data: 2
			}
		];
	}
);

// This handler resolves additional information for the item selected in
// the completion list.
connection.onCompletionResolve(
	(item: CompletionItem): CompletionItem => {
		if (item.data === 1) {
			item.detail = 'TypeScript details';
			item.documentation = 'TypeScript documentation';
		} else if (item.data === 2) {
			item.detail = 'JavaScript details';
			item.documentation = 'JavaScript documentation';
		}
		return item;
	}
);
*/

// Make the text document manager listen on the connection
// for open, change and close text document events
documents.listen(connection);

// Listen on the connection
connection.listen();
