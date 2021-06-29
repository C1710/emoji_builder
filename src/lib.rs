/*
 * Copyright 2019 Constantin A. <emoji.builder@c1710.de>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
//! The main crate for emoji_builder containing all the logic

#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

/// The module containing the [builder::EmojiBuilder]-trait which is used to implement new emoji-builders
pub mod builder;
/// Concrete builders (will be outsourced at some point)
pub mod builders;
/// A helper module to detect file changes based on their SHA256 hashes
pub mod changes;
/// Handling for single emojis
pub mod emoji;
/// Tables that contain metadata about emojis, like their kind and name
pub mod emoji_tables;
/// [emoji_processor::EmojiProcessor] is a trait for transformation functions that can work on e.g.
/// the SVG-representation of an emoji to modify it
/// (Subject to change)
pub mod emoji_processor;
/// Similar to `emoji_processor`, but creating new, derived emojis.
/// Currently WIP
pub mod deriving_emoji_processor;
/// Concrete emoji processors
pub mod emoji_processors;
pub mod packs;
pub mod loadable;
pub mod configs;


#[cfg(test)]
mod tests;