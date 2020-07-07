/*
 * Copyright 2020 Constantin A. <emoji.builder@c1710.de>.
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
use clap::{App, ArgMatches, SubCommand};
use crate::emoji::Emoji;
use usvg::Tree;

struct Waveflag {
    verbose: bool
}

impl EmojiProcessor<usvg::Tree> for Waveflag {
    type Err = ();

    fn new(verbose: bool, _: Option<ArgMatches>) -> Result<Box<Self>, Self::Err> {
        Ok(Box::new(Waveflag {
            verbose
        }))
    }

    fn process(&self, _emoji: &Emoji, prepared: Tree) -> Result<Tree, Self::Err> {
        let root = prepared.svg_node();
        root.

        Ok(prepared)
    }

    fn sub_command<'a, 'b>() -> App<'a, 'b> {
        SubCommand::with_name("waveflag")
    }
}
