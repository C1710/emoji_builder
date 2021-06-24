# `gimp_palette`

 Converts RGB color values to a GIMP gpl palette 

## License

MIT

## Usage

```rust
extern crate gimp_palette;

fn main() {
    let colors = vec![ gimp_palette::Color { r: 0, g: 50, b: 255 } ];
    let palette = gimp_palette::Palette::new("Example", colors).unwrap();
    palette.write_to_file("test.gpl").expect(&format!("Failed to write {} to test.gpl", palette.get_name()));
}
```