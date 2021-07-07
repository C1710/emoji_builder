/// Tables that contain metadata about emojis, like their kind and name
pub mod emoji_tables;
pub mod errors;
#[cfg(feature = "online")]
pub mod online;
#[cfg(test)]
mod tests;