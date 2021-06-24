/*
 * Copyright 2021 Constantin A. <emoji.builder@c1710.de>
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

use crate::emoji_processor::EmojiProcessor;
use crate::emoji::Emoji;

// TODO: Enforce documentation

// Implementation is not ready at the moment anyway
#[allow(missing_docs)]
pub trait DerivingEmojiProcessor<T>: Send + Sync + EmojiProcessor<T> {
    type DerivationTag: Clone + Send + Sync;
    /// Returns a list of the emojis this processor can derive from the given emoji
    fn derivations(&self, emoji: &Emoji) -> Option<DerivedEmojis<Self::DerivationTag>>;

    /// Actually creates the derivations of the given emoji
    fn derive(&self, derivations: DerivedEmojis<Self::DerivationTag>, prepared: T) -> Vec<(Emoji, T)>;
}


// Implementation is not ready at the moment anyway
#[allow(missing_docs)]
pub struct DerivedEmojis<T> where T: Clone + Send + Sync {
    pub base: Emoji,
    pub derived: Vec<(Emoji, T)>
}