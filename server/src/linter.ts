import ffi = require('ffi-napi');
import ref = require('ref-napi');
import StructType = require('ref-struct-napi');
import ArrayType = require('ref-array-napi');

// Initialize the C-like array
const OutputArrayType = ArrayType(ref.types.int64, 2);

// TODO: set correct output type

// Initialize the C-like data struct
const OutputType = StructType({
  result: ref.types.int64,
  operands: OutputArrayType,
  description: ref.types.CString
});


// Accessing the library
// See its signature https://github.com/node-ffi/node-ffi/wiki/Node-FFI-Tutorial#signature
const lib = ffi.Library('linter-wrapper/target/debug/liblinter_wrapper', {
	lint_file: ['int', ['int']] // TODO: set correct function typing
});

//TODO: export calls to be used in server.ts