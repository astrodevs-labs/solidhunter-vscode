import ffi = require('ffi-napi');
import ref = require('ref-napi');
//import StructType = require('ref-struct-napi');
var ArrayType = require('ref-array-di')(ref);
var StructType = require('ref-struct-di')(ref);

// Initialize the C-like data struct
const VSSolid_Diag = StructType({
    empty: ref.types.bool,
    start_line: ref.types.int64,
    start_char: ref.types.int64,
    end_line: ref.types.int64,
    end_char: ref.types.int64,
    severity: ref.types.int64,
    message: ref.types.CString
});

// Initialize the C-like array
const DiagArray = ArrayType(VSSolid_Diag, 100);





// Accessing the library
// See its signature https://github.com/node-ffi/node-ffi/wiki/Node-FFI-Tutorial#signature
const lib = ffi.Library('linter-wrapper/target/debug/liblinter_wrapper', {
    lint_file: [DiagArray, ['string']],
    lint_content: [DiagArray, ['string', 'string']]
});

export default class Linter {
    public static lintFile(filePath: string) {
        // Call the library
        const result = lib.lint_file(filePath);
        // convert or return ? 
    }

    public static lintContent(path: string, content: string) {
        // Call the library
        const result = lib.lint_content(path, content);
        // convert or return ?
    }
}