# super-simd

A very simple library to cover anyone's need to add lots of `u8`s together. It gives access to a single new type: `u8x512`, which represents a vector of 512 `u8`s.

## Trait implementations

The following implementations are given by the library:

```rust
impl Copy for u8x512;
impl Clone for u8x512;
impl Debug for u8x512;
impl From<&[u8; 512]> for u8x512;
impl Into<Box<[u8; 512]>> for &u8x512;
impl Add<&u8x512> for &u8x512;
impl Add<u8x512> for u8x512;
impl AddAssign<&u8x512> for u8x512;
impl AddAssign<u8x512> for u8x512;
```

## Example

You can find a working example in `examples/demo.rs`, run it with:

```bash
cargo run --example demo --release
```

Here is an overview of `demo.rs`:

```rust
extern crate super_simd;
use super_simd::u8x512;

const DATA1: [u8; 512] = [...];
const DATA2: [u8; 512] = [...];

fn main() {
    let arr1 = u8x512::from(&DATA1);
    let arr2 = u8x512::from(&DATA2);

    println!("array1 = {:?}\n\narray2 = {:?}\n", arr1, arr2);

    let res = arr1 + arr2;

    println!("result = {:?}", res);
}
```

## Benchmarking

To run the benchmarks, enter the following command:
```bash
cargo bench
```

The results below were obtained with an Intel® Core™ i5-8250U running Ubuntu 20.04.1 LTS 64-bits and with rustc version 1.46.0-nightly.

This processor has SSE and SSE2 capabilities, and is therefore able to perform 128-bits simd operations.

```
test tests::scalar     ... bench:          19 ns/iter (+/- 0)
test tests::super_simd ... bench:           9 ns/iter (+/- 0)
```

The `scalar` benchmark performs the additions one after the other and place the result in an array. The `super_simd` benchmark performs the additions the "super-simd way"  (see section "Design").

These numbers give a 19/9=2.11 speedup gained by using the library over adding `u8s` the "naive" way.

## Design

### Addition algorithm

This library makes use of the long addition algorithm for binary numbers. Let's denote the xor operator by `^`, the and operator by `&` and the or operator by `|`. let `a` and `b` be our two operands, `carry` a temporary variable and `res` the variable holding the result of `a + b`.

```
carry <- 0
for i <- 1 to N do
    res[i] <- a[i] ^ b[i] ^ carry
    carry <- (a[i] & b[i]) | (a[i] & carry) | (b[i] & carry)
end
```

Those operations are carried on 512 bits at a time (kind of) thanks to the simd instructions set.

### Verticalization

First, take the array of 512 `u8`s:
```
[10100110, 11001001, 00000000, 11111111, ...]
```
Then, place them in colums in an array of eight 512 bits elements (represented in the code by a tuple of eight `u64`s):

```
[ 1101... ,
  0101... ,
  1001... ,
  0001... ,
  0101... ,
  1001... ,
  1001... ,
  0101...   ]
```

Then we can apply the long addition algorithm to each massive elements of that array as if there were each a digit of a single binary number. To then get the result back into usable format, simply put those columns back into an array.

## Performance and limitations

Since tests couldn't be carried on a 512-bits simd enabled processor, the scalar method of adding `u8`s together might become faster with larger regular simd registers. Or not, maybe the super-simd method gets faster. This needs testing.

Also, while optimized and branchless, the `from` function and `into` method are relativly slow compared to an addition, so having to switch constantly between array form and `u8x512` form will severly impact performance. It may be necessary to carry dozens, or maybe even hundreds, of additions on `u8x512`s between the conversions from and into an array to get a perceivable performance gain.
