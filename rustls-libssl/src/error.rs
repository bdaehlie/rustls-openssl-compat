use core::ffi::{c_int, c_long};
use core::ptr;
use std::ffi::{CStr, CString};

use openssl_sys::{ERR_new, ERR_set_error, ERR_RFLAGS_OFFSET, ERR_RFLAG_FATAL};

// See openssl/err.h for the source of these magic numbers.

#[derive(Copy, Clone, Debug)]
#[repr(i32)]
enum Lib {
    /// This is `ERR_LIB_SSL`.
    Ssl = 20,

    /// This is `ERR_LIB_USER`.
    User = 128,
}

const ERR_RFLAG_COMMON: i32 = 0x2i32 << ERR_RFLAGS_OFFSET;

#[derive(Copy, Clone, Debug)]
#[repr(i32)]
enum Reason {
    PassedNullParameter = (ERR_RFLAG_FATAL as i32) | ERR_RFLAG_COMMON | 258,
    InternalError = (ERR_RFLAG_FATAL as i32) | ERR_RFLAG_COMMON | 259,
    UnableToGetWriteLock = (ERR_RFLAG_FATAL as i32) | ERR_RFLAG_COMMON | 272,
    OperationFailed = (ERR_RFLAG_FATAL as i32) | ERR_RFLAG_COMMON | 263,
    Unsupported = ERR_RFLAG_COMMON | 268,
}

#[derive(Debug)]
pub struct Error {
    lib: Lib,
    reason: Reason,
    string: Option<String>,
}

impl Error {
    pub fn unexpected_panic() -> Self {
        Self {
            lib: Lib::Ssl,
            reason: Reason::InternalError,
            string: None,
        }
    }

    pub fn null_pointer() -> Self {
        Self {
            lib: Lib::Ssl,
            reason: Reason::PassedNullParameter,
            string: None,
        }
    }

    pub fn cannot_lock() -> Self {
        Self {
            lib: Lib::Ssl,
            reason: Reason::UnableToGetWriteLock,
            string: None,
        }
    }

    pub fn not_supported(hint: &str) -> Self {
        Self {
            lib: Lib::Ssl,
            reason: Reason::Unsupported,
            string: Some(hint.to_string()),
        }
    }

    pub fn bad_data(hint: &str) -> Self {
        Self {
            lib: Lib::Ssl,
            reason: Reason::OperationFailed,
            string: Some(hint.to_string()),
        }
    }

    pub fn from_rustls(err: rustls::Error) -> Self {
        Self {
            lib: Lib::User,
            reason: Reason::OperationFailed,
            string: Some(err.to_string()),
        }
    }

    pub fn from_io(err: std::io::Error) -> Self {
        Self {
            lib: Lib::User,
            reason: Reason::OperationFailed,
            string: Some(err.to_string()),
        }
    }

    /// Add this error to the openssl error stack.
    pub fn raise(self) -> Self {
        log::error!("raising {self:?}");
        let cstr = CString::new(
            self.string
                .clone()
                .unwrap_or_else(|| format!("{:?}", self.reason)),
        )
        .unwrap();
        // safety: b"%s\0" satisfies requirements of from_bytes_with_nul_unchecked.
        let fmt = unsafe { CStr::from_bytes_with_nul_unchecked(b"%s\0") };
        unsafe {
            ERR_new();
            // nb. miri cannot do variadic functions, so we define a miri-only equivalent
            #[cfg(not(miri))]
            ERR_set_error(
                self.lib as c_int,
                self.reason as c_int,
                fmt.as_ptr(),
                cstr.as_ptr(),
            );
            #[cfg(miri)]
            crate::miri::ERR_set_error(self.lib as c_int, self.reason as c_int, cstr.as_ptr());
        }
        self
    }
}

// These conversions determine how errors are reported from entry point
// functions.

impl<T> From<Error> for *const T {
    fn from(_: Error) -> Self {
        ptr::null()
    }
}

impl<T> From<Error> for *mut T {
    fn from(_: Error) -> Self {
        ptr::null_mut()
    }
}

impl From<Error> for c_int {
    fn from(_: Error) -> Self {
        // for typical OpenSSL functions (return 0 on error)
        0
    }
}

impl From<Error> for c_long {
    fn from(_: Error) -> Self {
        // ditto
        0
    }
}

impl From<Error> for u64 {
    fn from(_: Error) -> Self {
        // for options functions (return 0 on error)
        0
    }
}

impl From<Error> for u32 {
    fn from(_: Error) -> Self {
        // for `SSL_CIPHER_get_id`
        0
    }
}

impl From<Error> for u16 {
    fn from(_: Error) -> Self {
        // for `SSL_CIPHER_get_protocol_id`
        0
    }
}

impl From<Error> for () {
    fn from(_: Error) {
        // for void functions (return early on error)
    }
}

#[macro_export]
macro_rules! ffi_panic_boundary {
    ( $($tt:tt)* ) => {
        match ::std::panic::catch_unwind(
            ::std::panic::AssertUnwindSafe(|| {
                $($tt)*
        })) {
            Ok(ret) => ret,
            Err(_) => return $crate::error::Error::unexpected_panic()
                .raise()
                .into(),
        }
    }
}

pub(crate) use ffi_panic_boundary;
