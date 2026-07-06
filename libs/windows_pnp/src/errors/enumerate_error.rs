// Copyright (c) ScaleFS LLC; used with permission
// Licensed under the MIT License

#[derive(Debug)]
pub enum EnumerateError {
    StringDecodingError(/*error: */std::string::FromUtf16Error),
    StringTerminationDecodingError,
    Win32Error(/*win32_error: */u32),
    Win32ErrorInvalidData(/*win32_error_invalid_data */String)
}

