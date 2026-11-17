pub mod external;
pub mod http;
pub mod lastfm;
pub mod lyrics;
pub mod url;
pub mod ytdlp;

pub use external::ExternalResolver;
pub use lastfm::LastFmClient;
pub use url::{ParsedInput, key_from_input, parse_input};
pub use ytdlp::YtDlp;
