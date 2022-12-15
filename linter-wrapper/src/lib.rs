extern crate libc;

use libc::c_char;

use std::ffi::CStr;
use std::ffi::CString;
use solidhunter_lib::types::*;
use solidhunter_lib::linter::*;

#[repr(C)]
#[derive(Debug)]
pub struct VSSolid_Diag {
    pub empty: bool,
    pub start_line: i64,
    pub start_char: i64,
    pub end_line: i64,
    pub end_char: i64,
    pub severity: i64,
    pub message: *const c_char
}

impl VSSolid_Diag {
    pub fn from(diag: LintDiag) -> VSSolid_Diag {
        VSSolid_Diag { 
            empty: false,
            start_line: diag.range.start.line as i64,
            start_char: diag.range.start.character as i64,
            end_line: diag.range.end.line as i64,
            end_char: diag.range.end.character as i64,
            severity: diag.severity.unwrap() as i64,
            message: CString::new(diag.message).unwrap().into_raw()
        }
    }
/*
    pub fn from(err: LintError) -> VSSolid_Diag {
        match err {
            LintError::SolcError(e) => {
                VSSolid_Diag {
                    empty: false,
                    start_line: e.  as i64,
                    start_char: diag.range.start.character as i64,
                    end_line: diag.range.end.line as i64,
                    end_char: diag.range.end.character as i64,
                    severity: diag.severity.unwrap() as i64,
                    message: CString::new(diag.message).unwrap().into_raw()
                }
            },
            LintError::IoError(e) => todo!(),
            LintError::LinterError(e) => todo!(),
        }

    }
    */

    pub fn empty() -> VSSolid_Diag {
        VSSolid_Diag { 
            empty: false,
            start_line: -1,
            start_char: -1,
            end_line: -1,
            end_char: -1,
            severity: -1,
            message: CString::new("").unwrap().into_raw()
        }
    }
}



pub extern fn lint_file(path: *const c_char, config: *const c_char) -> [VSSolid_Diag; 100] {
    let mut diags : Vec<VSSolid_Diag> = Vec::new();
    let input_cstring: &CStr = unsafe {
        // Wraps a raw C-string with a safe C string wrapper
        // Function is unsafe
        CStr::from_ptr(config)
    };

    let path_cstring: &CStr = unsafe {
        CStr::from_ptr(path)
    };
    // Converts a valid UTF-8 CStr into a string slice
    let config_str: &str = input_cstring.to_str().unwrap();
    let path_str: &str = path_cstring.to_str().unwrap();
    let mut linter : SolidLinter = SolidLinter::new();
    let config_string = String::from(config_str);
    linter.initalize(&config_string);

    let lint_result = linter.parse_file(String::from(path_str));
    
    match lint_result {
        Ok(solid_diags) => {
            let mut nb = 0;
            for diag in solid_diags {
                let res_diag = VSSolid_Diag::from(diag);
                diags.push(res_diag);
                nb += 1;
            }
            loop {
                if nb == 99 {
                    break;
                }
                diags.push(VSSolid_Diag::empty());
                nb += 1;
            }
        },
        Err(err) => {
            let mut nb = 1;
            diags.push(VSSolid_Diag { 
                empty: false,
                start_line: 0,
                start_char: 0,
                end_line: 0,
                end_char: 1,
                severity: Severity::ERROR as i64,
                message: CString::new(err.to_string()).unwrap().into_raw()
            });
            loop {
                if nb == 99 {
                    break;
                }
                diags.push(VSSolid_Diag::empty());
                nb += 1;
            }
        },
    }
    diags.try_into().unwrap()

}