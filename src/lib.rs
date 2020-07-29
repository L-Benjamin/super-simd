#![feature(repr_simd, platform_intrinsics, test)]
#![allow(non_camel_case_types)]

use std::fmt::Debug;
use std::mem::{MaybeUninit, transmute};
use std::ops::{Add, AddAssign};

//#################################################################################################
//
//                                         struct u64x8
//
//#################################################################################################

/*
 * A struct holding 8 u64s, for a total of 512 bits. The repr(simd) allows it to be held in
 * the special simd registers of your cpu, if there exists.
 */
#[repr(simd)]
#[derive(Clone, Copy)]
struct u64x8(u64, u64, u64, u64, u64, u64, u64, u64);

impl u64x8 {
    /*
     * The zero of a u64x8 (8 zeros, really).
     */
    const ZERO: u64x8 = u64x8(0, 0, 0, 0, 0, 0, 0, 0);
}

/*
 * Get the functions allowing the use of your cpu's simd capabilities.
 */
extern "platform-intrinsic" {
    fn simd_xor<u64x8>(a: u64x8, b: u64x8) -> u64x8;
    fn simd_and<u64x8>(a: u64x8, b: u64x8) -> u64x8;
    fn simd_or<u64x8>(a: u64x8, b: u64x8) -> u64x8;
}

//#################################################################################################
//
//                                      struct u8x128
//
//#################################################################################################

/*
 * The struct defining a u8x512, 8 rows of u64x8s, for a total of 4096 bits.
 */
#[derive(Clone, Copy)]
pub struct u8x512 {
    rows: [u64x8; 8],
}

/*
 * Converts a reference to an array of 512 u8s to a u8x512. The conversion is done through
 * "horizontalization". Think of it as a matrix transposition.
 */
impl From<&[u8; 512]> for u8x512 {
    fn from(cols: &[u8; 512]) -> u8x512 {
        let mut rows = [u64x8::ZERO; 8];
        let mut h_mask: u64;

        macro_rules! horizontalize {
            ($row: tt, $range: expr) => {
                h_mask = 1;

                for i in $range {
                    let mut col = cols[i].clone();

                    for _ in 0..col.count_ones() {
                        let v = col.trailing_zeros();
                        col ^= 1 << v;
                        rows[v as usize].$row |= h_mask;
                    }

                    h_mask = h_mask.wrapping_shl(1);
                }
            }
        }

        horizontalize!(0, 0..64);
        horizontalize!(1, 64..128);
        horizontalize!(2, 128..192);
        horizontalize!(3, 192..256);
        horizontalize!(4, 256..320);
        horizontalize!(5, 320..384);
        horizontalize!(6, 384..448);
        horizontalize!(7, 448..512);

        u8x512 {rows}
    }
}

/*
 * Converts a u8x512 to a boxed array of u8s. The algorithm is basically the
 * same as the one of used by the above function.
 */
impl Into<Box<[u8; 512]>> for &u8x512 {
    fn into(self) -> Box<[u8; 512]> {
        let mut cols = [0; 512];
        let mut v_mask: u8 = 1;

        for row in self.rows.iter() {
            let mut row_cpy;

            macro_rules! verticalize {
                ($row: tt, $offset: expr) => {
                    row_cpy = row.$row.clone();

                    for _ in 0..row_cpy.count_ones() {
                        let h = row_cpy.trailing_zeros();
                        row_cpy ^= 1 << h;
                        cols[(h + $offset) as usize] |= v_mask;
                    }
                }
            }

            verticalize!(0, 0);
            verticalize!(1, 64);
            verticalize!(2, 128);
            verticalize!(3, 192);
            verticalize!(4, 256);
            verticalize!(5, 320);
            verticalize!(6, 384);
            verticalize!(7, 448);

            v_mask = v_mask.wrapping_shl(1);
        }

        Box::new(cols)
    }
}

/*
 * Implementation of Display for u8x512, formats it like a Vec<u8>.
 */
impl Debug for u8x512 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let numbers: Box<[u8; 512]> = self.into();
        numbers.to_vec().fmt(f)
    }
}

/*
 * Implementation of &u8x512 + &u8x512 -> u8x512. It's an implementation of the binary long
 * addition algorithm, done 512 times in parallel. Considering a simd operation as a
 * single operation, the complexity is equal to 2+7x6+2 = 46 operations, giving a theoretical
 * speedup of 512/46 = 11.13
 */
impl Add for &u8x512 {
    type Output = u8x512;

    fn add(self, rhs: &u8x512) -> u8x512 {
        unsafe {
            let mut res: [MaybeUninit<u64x8>; 8] = MaybeUninit::uninit().assume_init();

            res[0] = MaybeUninit::new(simd_xor(self.rows[0], rhs.rows[0]));
            let mut carry = simd_and(self.rows[0], rhs.rows[0]);

            for i in 1..7 {
                res[i] = MaybeUninit::new(simd_xor(simd_xor(
                    self.rows[i],
                    rhs.rows[i]),
                    carry,
                ));

                carry = simd_or(simd_or(
                    simd_and(self.rows[i], rhs.rows[i]),
                    simd_and(self.rows[i], carry)),
                    simd_and(rhs.rows[i], carry),
                );
            }

            res[7] = MaybeUninit::new(simd_xor(simd_xor(
                self.rows[7],
                rhs.rows[7]),
                carry,
            ));

            u8x512 {rows: transmute(res)}
        }
    }
}

/*
 * Implementation of u8x512 + u8x512 -> u8x512.
 */
impl Add for u8x512 {
    type Output = u8x512;

    #[inline(always)]
    fn add(self, rhs: u8x512) -> u8x512 {
        &self + &rhs
    }
}

/*
 * Implementation of &mut u8x512 += &u8x512.
 */
impl AddAssign<&u8x512> for u8x512 {
    fn add_assign(&mut self, rhs: &u8x512) {
        unsafe {
            let mut tmp;
            let mut carry = simd_and(self.rows[0], rhs.rows[0]);
            self.rows[0] = simd_xor(self.rows[0], rhs.rows[0]);

            for i in 1..7 {
                tmp = simd_xor(simd_xor(
                    self.rows[i],
                    rhs.rows[i]),
                    carry,
                );

                carry = simd_or(simd_or(
                    simd_and(self.rows[i], rhs.rows[i]),
                    simd_and(self.rows[i], carry)),
                    simd_and(rhs.rows[i], carry),
                );

                self.rows[i] = tmp;
            }

            self.rows[7] = simd_xor(simd_xor(
                self.rows[7],
                rhs.rows[7]),
                carry,
            );
        }
    }
}

/*
 * Implementation of &mut u8x512 += u8x512.
 */
impl AddAssign for u8x512 {
    fn add_assign(&mut self, rhs: u8x512) {
        *self += &rhs;
    }
}

//#################################################################################################
//
//                                         mod tests
//
//#################################################################################################

#[cfg(test)]
mod tests {
    extern crate test;
    use test::Bencher;

    use std::mem::{MaybeUninit, transmute};

    use super::u8x512;

    /*
     * The seed used to fill the arrays before the benchmarks.s
     */
    const SEED: u32 = 123;

    /*
     * Fills an array of 512 u8s with pseudo-random integers. Uses the xorshift32 algorithm to
     * generate pseudo-random 32 bits integers.
     */
    fn init_array(state: &mut u32) -> [u8; 512] {
        fn xorshift32(state: &mut u32) -> u32 {
            let mut x = *state;
            x ^= x.wrapping_shl(13);
            x ^= x.wrapping_shr(17);
            x ^= x.wrapping_shl(5);
            *state = x;
            x
        }

        let mut res = [0; 512];

        for i in 0..512 {
            res[i] = (xorshift32(state) & 0xFF) as u8;
        }

        res
    }

    /*
     * Tests the correctness of the addition of two u8x512s. Results are compared with
     * cpu's additions.
     */
    #[test]
    fn add() {
        let mut state = SEED;
        let a1 = init_array(&mut state);
        let a2 = init_array(&mut state);

        let a1_ssimd = u8x512::from(&a1);
        let a2_ssimd = u8x512::from(&a2);

        let res_ssimd = a1_ssimd + a2_ssimd;
        let res: Box<[u8; 512]> = (&res_ssimd).into();

        for i in 0..512 {
            assert_eq!(res[i], a1[i].wrapping_add(a2[i]));
        }
    }

    /*
     * Tests the correctness of the addition assignement of two u8x512s. Results are compared
     * with cpu's additions.
     */
    #[test]
    fn add_assign() {
        let mut state = SEED;
        let a1 = init_array(&mut state);
        let a2 = init_array(&mut state);

        let mut a1_ssimd = u8x512::from(&a1);
        let a2_ssimd = u8x512::from(&a2);

        a1_ssimd += a2_ssimd;

        let res: Box<[u8; 512]> = (&a1_ssimd).into();

        for i in 0..512 {
            assert_eq!(res[i], a1[i].wrapping_add(a2[i]));
        }
    }

    /*
     * Benchmarks the time it takes to add 512 u8s with simple additions.
     */
    #[bench]
    fn scalar(b: &mut Bencher) {
        let mut state = SEED;
        let a1 = init_array(&mut state);
        let a2 = init_array(&mut state);

        b.iter(|| unsafe {
            let mut res: [MaybeUninit<u8>; 512] = MaybeUninit::uninit().assume_init();

            for i in 0..512 {
                res[i] = MaybeUninit::new(a1[i].wrapping_add(a2[i]));
            }

            transmute::<_, [u8; 512]>(res)
        })
    }

    /*
     * Benchmarks the time it takes to add two u8x512s (not including the time it
     * takes to convert from and into an array).
     */
    #[bench]
    fn super_simd(b: &mut Bencher) {
        let mut state = SEED;
        let a1 = u8x512::from(&init_array(&mut state));
        let a2 = u8x512::from(&init_array(&mut state));

        b.iter(|| {
            a1 + a2
        })
    }
}
