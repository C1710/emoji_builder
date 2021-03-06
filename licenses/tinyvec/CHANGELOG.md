# Changelog

## 1.1.0

* [slightlyoutofphase](https://github.com/slightlyoutofphase)
added "array splat" style syntax to the `array_vec!` and `tiny_vec!` macros.
You can now write `array_vec![true; 5]` and get a length 5 array vec full of `true`,
just like normal array initialization allows. Same goes for `tiny_vec!`.
([pr 118](https://github.com/Lokathor/tinyvec/pull/118))
* [not-a-seagull](https://github.com/not-a-seagull)
added `ArrayVec::into_inner` so that you can get the array out of an `ArrayVec`.
([pr 124](https://github.com/Lokathor/tinyvec/pull/124))

## 1.0.2

* Added license files for the MIT and Apache-2.0 license options.

## 1.0.1

* Display additional features in the [docs.rs/tinyvec](https://docs.rs/tinyvec) documentation.

## 1.0.0

Initial Stable Release.
