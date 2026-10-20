//! The values a saved chat's records carry, shared by the readers in
//! [`crate::history`] and the chat events in [`crate::chats`]. They need no
//! transport, so a browser build reads them too.
use serde::Deserialize;

macro_rules! open_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone,Debug,PartialEq,Eq)]
        pub enum $name { $($variant,)+ Other(String) }
        impl $name {
            /// The value as the service writes it.
            pub fn as_str(&self)->&str { match self {$(Self::$variant=>$wire,)+Self::Other(s)=>s} }
        }
        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error> {
                Ok(match String::deserialize(d)?.as_str() { $($wire=>Self::$variant,)+s=>Self::Other(s.to_owned()) })
            }
        }
    }
}
open_enum!(
    /// Where a chat started. `Other` holds a value this SDK doesn't know.
    ChatSource { Web=>"web", Terminal=>"terminal", Phone=>"phone" });
open_enum!(
    /// Who wrote a saved message. `Other` holds a value this SDK doesn't know.
    MessageRole { User=>"user", Assistant=>"assistant", Tool=>"tool", Thinking=>"thinking" });
open_enum!(
    /// Whether a tool call is running, done, or failed.
    ToolStatus { Running=>"running", Done=>"done", Failed=>"failed" });
open_enum!(
    /// The type of a saved file.
    FileKind { Png=>"png", Jpeg=>"jpeg", Gif=>"gif", Webp=>"webp", Pdf=>"pdf", Text=>"text", Step=>"step", Stl=>"stl" });
open_enum!(
    /// Whether a capture's media was saved.
    CaptureState { Saved=>"saved", Unavailable=>"unavailable", NotSaved=>"not_saved" });
