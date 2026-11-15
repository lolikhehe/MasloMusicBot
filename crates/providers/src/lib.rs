pub mod external;
pub mod url;
pub mod ytdlp;

pub use external::ExternalResolver;
pub use url::{ParsedInput, key_from_input, parse_input};
pub use ytdlp::YtDlp;
