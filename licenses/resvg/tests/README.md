# SVG tests

This directory contains a collection of SVG files used during *resvg* regression testing.

## Adding a new test

### Select a correct name

Each test has a `type-name-index.svg` format.

- `type` can be either an `a`(attribute) or an `e`(element)
- `name` corresponds to an actual SVG attribute or element
- `index` is just a serial number

### Create an SVG file

We are using SVG files with a fixed, 200x200 viewbox for all tests.

Here is a test file template:

```xml
<svg id="svg1" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
    <title>My new test</title>

    <!-- replace with an actual SVG data -->

    <!-- image frame -->
    <rect id="frame" x="1" y="1" width="198" height="198" fill="none" stroke="black"/>
</svg>

```

General requirements:

1. Each test must test only a single issue.
1. Each element must have an `id` attribute.
1. The `title` value must be unique and shorter than 60 characters.<br/>
   Newlines are not allowed.
1. Each line in an XML file should be less than 100 characters.
1. No trailing spaces.
1. A single trailing newline.
1. UTF-8 only.

You could use the `check.py` script to automatically check those requirements.

### Render PNG

After the SVG test is finished, you should render it using resvg:

```sh
cargo run --release -- \
    --width 300 \
    --skip-system-fonts \
    --use-fonts-dir 'tests/fonts' \
    --font-family 'Noto Sans' \
    --serif-family 'Noto Serif' \
    --sans-serif-family 'Noto Sans' \
    --cursive-family 'Yellowtail' \
    --fantasy-family 'Sedgwick Ave Display' \
    --monospace-family 'Noto Mono' \
    in.svg out.png
```

(we are using 300px width to test scaling)

After that, you should optimize the resulting PNG using oxipng:

```sh
cargo install oxipng
oxipng -o 6 -Z out.png
```

And then place it into the `png` dir.

## License

MIT

The library itself is under the MPL2.0, but tests are under MIT,
so you can do whatever you want with them.
