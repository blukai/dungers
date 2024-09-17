//! this is a partial port of valve's bitbuf. original implementation can be found on github
//! <https://github.com/ValveSoftware/source-sdk-2013>.

mod bitreader;
mod bitwriter;
mod common;
mod error;

pub use bitreader::BitReader;
pub use bitwriter::BitWriter;
pub use common::get_bit_for_bit_num;
pub use error::Error;

pub(crate) use common::*;
