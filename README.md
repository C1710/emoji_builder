[![Build Status](https://travis-ci.com/C1710/emoji_builder.svg?token=Mr9kkSveUkaSSi3GNLyz&branch=dev)](https://travis-ci.com/C1710/emoji_builder)

# Emoji Builder
_Currently under development_.

## Build
You will need a working Rust toolchain and `clang` and (maybe?) `cmake`.  
Install instructions for the Rust toolchain can be found at https://rustup.rs.  
Install instructions for `cmake` can be found at https://cmake.org

If you use Windows 10, you might want to use [`winget`](https://github.com/microsoft/winget-cli) for that:
```
winget install rustup
winget install -e CMake
winget install clang
```
(You might need to add `cmake` and `clang` to your PATH first. When installed on Windows using `winget` it's usually located at `C:\Program Files\CMake\bin` and `C:\Program Files\LLVM\bin`)

You'll also need a working Python 3.6+ environment with some additional requirements: https://pyo3.rs/v0.11.0/building_and_distribution.html#linking

If everything is installed successfully you can simply run `cargo build`, `cargo run`, `cargo test`, etc. and that's it.  

## License
    Copyright 2019-2020 Constantin A. <emoji.builder@c1710.de>

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

       http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.

## Third party code
This project uses many different crates. Their licenses can be found in the `licenses` folder.  
For more information, there's a `README.txt` included.  
However, only licenses for dependencies are included that are neither `dev-`, nor `build-`dependencies.  
Anyway, the whole source code of the dependencies is available online and also locally once `cargo build` is called.