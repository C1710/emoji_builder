[![Travis CI build status](https://travis-ci.org/C1710/emoji_builder.svg?branch=master)](https://travis-ci.org/C1710/emoji_builder) ![GitHub Workflow build status](https://github.com/C1710/emoji_builder/workflows/Rust/badge.svg)

# Emoji Builder
_Currently under development_.

## Build
You will need a working Rust toolchain and Python >=3.6.  
Install instructions for the Rust toolchain can be found at https://rustup.rs.  

If you use Windows 10, you might want to use [`winget`](https://github.com/microsoft/winget-cli) for that:
```
winget install rustup
winget install -e Python.Python
```

You'll also have to provide the appropriate libraries (note: it has to be the libraries for whichever version `python` refers to): https://pyo3.rs/v0.11.1/building_and_distribution.html#linking  
For example, if you installed Python 3.9 via `winget`, it should be in your `AppData\\Programs`-directory.  
You can then set the environment variable as follows (please note that this will overwrite your `LIB` environment variable):
```
setx LIB C:\Users\<YourUserName>\AppData\Local\Programs\Python\Python39\libs\python39.lib
```

Currently you are also required to have `fonttools` and `notofonttools` installed in Python (you might want to use a venv for that),
these can be installed by running `python -m pip install -r requirements.txt` (if you are in the root directory of this project)

Unfortunately, you'll have to provide such a Python installation even for the compiled executables, while `clang` is only required for building.

If everything is installed successfully you can simply run `cargo build`, `cargo run`, `cargo test`, etc.  

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
