## Cf-Zlib

This is Rust [sys crate](http://kornel.ski/rust-sys-crate) for [Cloudflare's SIMD-accelerated fork of zlib](https://github.com/cloudflare/zlib).

It requires x86-64 CPU with SSE 4.2 or ARM64 with NEON & CRC. It does not support 32-bit CPUs at all.

This crate has the same API as [zlib](https://zlib.net/), and conflicts with [libz-sys](https://crates.rs/crates/libz-sys).

Note: you will have to ensure that the program using `cloudflare-zlib-sys` does not link with any other version of `libz`. Otherwise the accelerated version may not be used, or the program could even crash. Because of [a Cargo issue](https://github.com/rust-lang/cargo/issues/6231) this crate doesn't prevent this problem.

## Cloning

This repository uses git submodules, so when cloning make sure to add `--recursive`

    git clone --recursive https://gitlab.com/kornelski/cloudflare-zlib-sys

If you cloned without `--recursive` you can fix it with:

    git submodule update --init


## Licenses

### Zlib

(C) 1995-2017 Jean-loup Gailly and Mark Adler

This software is provided 'as-is', without any express or implied
warranty.  In no event will the authors be held liable for any damages
arising from the use of this software.

Permission is granted to anyone to use this software for any purpose,
including commercial applications, and to alter it and redistribute it
freely, subject to the following restrictions:

1. The origin of this software must not be misrepresented; you must not
  claim that you wrote the original software. If you use this software
  in a product, an acknowledgment in the product documentation would be
  appreciated but is not required.
2. Altered source versions must be plainly marked as such, and must not be
  misrepresented as being the original software.
3. This notice may not be removed or altered from any source distribution.

Jean-loup Gailly jloup@gzip.org
Mark Adler madler@alumni.caltech.edu

If you use the zlib library in a product, we would appreciate *not* receiving
lengthy legal documents to sign.  The sources are provided for free but without
warranty of any kind.  The library has been entirely written by Jean-loup
Gailly and Mark Adler; it does not include third-party code.

If you redistribute modified sources, we would appreciate that you include in
the file ChangeLog history information documenting your changes.  Please read
the FAQ for more information on the distribution of modified source versions.

### libz-sys

This project is licensed under either of

  * [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0)
  * [MIT license](https://opensource.org/licenses/MIT)

at your option.
