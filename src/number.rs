use std::{ ops, fmt, f32, f64 };
use std::num::FpCategory;
use util::grisu2;
use util::print_dec;

/// NaN value represented in `Number` type. NaN is equal to itself.
pub const NAN: Number = Number {
    category: NAN_MASK,
    mantissa: 0,
    exponent: 0
};

const NEGATIVE: u8 = 0;
const POSITIVE: u8 = 1;
const NAN_MASK: u8 = !1;

/// Number representation used inside `JsonValue`. You can easily convert
/// the `Number` type into native Rust number types and back, or use the
/// equality operator with another number type.
///
/// ```
/// # use json::number::Number;
/// let foo: Number = 3.14.into();
/// let bar: f64 = foo.into();
///
/// assert_eq!(foo, 3.14);
/// assert_eq!(bar, 3.14);
/// ```
///
/// More often than not you will deal with `JsonValue::Number` variant that
/// wraps around this type, instead of using the methods here directly.
#[derive(Copy, Clone, Debug)]
pub struct Number {
    // A byte describing the sign and NaN-ness of the number.
    //
    // category == 0 (NEGATIVE constant)         -> negative sign
    // category == 1 (POSITIVE constnat)         -> positive sign
    // category >  1 (matches NAN_MASK constant) -> NaN
    category: u8,

    // Decimal exponent, analog to `e` notation in string form.
    exponent: i16,

    // Integer base before sing and exponent applied.
    mantissa: u64,
}

impl Number {
    /// Construct a new `Number` from parts. This can't create a NaN value.
    ///
    /// ```
    /// # use json::number::Number;
    /// let pi = Number::from_parts(true, 3141592653589793, -15);
    ///
    /// assert_eq!(pi, 3.141592653589793);
    /// ```
    #[inline]
    pub fn from_parts(positive: bool, mantissa: u64, exponent: i16) -> Self {
        Number {
            category: positive as u8,
            exponent: exponent,
            mantissa: mantissa,
        }
    }

    /// Reverse to `from_parts` - obtain parts from an existing `Number`.
    ///
    /// ```
    /// # use json::number::Number;
    /// let pi = Number::from(3.141592653589793);
    /// let (positive, mantissa, exponent) = pi.as_parts();
    ///
    /// assert_eq!(positive, true);
    /// assert_eq!(mantissa, 3141592653589793);
    /// assert_eq!(exponent, -15);
    /// ```
    #[inline]
    pub fn as_parts(&self) -> (bool, u64, i16) {
        (self.category == POSITIVE, self.mantissa, self.exponent)
    }

    #[inline]
    pub fn is_sign_positive(&self) -> bool {
        self.category == POSITIVE
    }

    #[inline]
    pub fn is_zero(&self) -> bool {
        self.mantissa == 0 && !self.is_nan()
    }

    #[inline]
    pub fn is_nan(&self) -> bool {
        self.category & NAN_MASK != 0
    }

    /// Test if the number is NaN or has a zero value.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.mantissa == 0 || self.is_nan()
    }

    /// Obtain an integer at a fixed decimal point. This is useful for
    /// converting monetary values and doing arithmetic on them without
    /// rounding errors introduced by floating point operations.
    ///
    /// Will return `None` if `Number` is negative or a NaN.
    ///
    /// ```
    /// # use json::number::Number;
    /// let price_a = Number::from(5.99);
    /// let price_b = Number::from(7);
    /// let price_c = Number::from(10.2);
    ///
    /// assert_eq!(price_a.as_fixed_point_u64(2), Some(599));
    /// assert_eq!(price_b.as_fixed_point_u64(2), Some(700));
    /// assert_eq!(price_c.as_fixed_point_u64(2), Some(1020));
    /// ```
    pub fn as_fixed_point_u64(&self, point: u16) -> Option<u64> {
        if self.category != POSITIVE {
            return None;
        }

        let e_diff = point as i16 + self.exponent;

        Some(if e_diff == 0 {
            self.mantissa
        } else if e_diff < 0 {
            self.mantissa.wrapping_div(decimal_power(-e_diff as u16))
        } else {
            self.mantissa.wrapping_mul(decimal_power(e_diff as u16))
        })
    }

    /// Analog to `as_fixed_point_u64`, except returning a signed
    /// `i64`, properly handling negative numbers.
    ///
    /// ```
    /// # use json::number::Number;
    /// let balance_a = Number::from(-1.49);
    /// let balance_b = Number::from(42);
    ///
    /// assert_eq!(balance_a.as_fixed_point_i64(2), Some(-149));
    /// assert_eq!(balance_b.as_fixed_point_i64(2), Some(4200));
    /// ```
    pub fn as_fixed_point_i64(&self, point: u16) -> Option<i64> {
        if self.is_nan() {
            return None;
        }

        let num = if self.is_sign_positive() {
            self.mantissa as i64
        } else {
            -(self.mantissa as i64)
        };

        let e_diff = point as i16 + self.exponent;

        Some(if e_diff == 0 {
            num
        } else if e_diff < 0 {
            num.wrapping_div(decimal_power(-e_diff as u16) as i64)
        } else {
            num.wrapping_mul(decimal_power(e_diff as u16) as i64)
        })
    }
}

impl PartialEq for Number {
    #[inline]
    fn eq(&self, other: &Number) -> bool {
        if self.is_zero() && other.is_zero()
        || self.is_nan()  && other.is_nan() {
            return true;
        }

        if self.category != other.category {
            return false;
        }

        let e_diff = self.exponent - other.exponent;

        if e_diff == 0 {
            return self.mantissa == other.mantissa;
        } else if e_diff > 0 {
            let power = decimal_power(e_diff as u16);

            self.mantissa.wrapping_mul(power) == other.mantissa
        } else {
            let power = decimal_power(-e_diff as u16);

            self.mantissa == other.mantissa.wrapping_mul(power)
        }

    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            if self.is_nan() {
                return f.write_str("nan")
            }
            let (positive, mantissa, exponent) = self.as_parts();
            let mut buf = Vec::new();
            print_dec::write(&mut buf, positive, mantissa, exponent).unwrap();
            f.write_str(&String::from_utf8_unchecked(buf))
        }
    }
}

fn exponent_to_power_f32(e: i16) -> f32 {
    static POS_POWERS: [f32; 16] = [
          1.0,    1e1,    1e2,    1e3,    1e4,    1e5,    1e6,    1e7,
          1e8,    1e9,   1e10,   1e11,   1e12,   1e13,   1e14,   1e15
    ];

    static NEG_POWERS: [f32; 16] = [
          1.0,   1e-1,   1e-2,   1e-3,   1e-4,   1e-5,   1e-6,   1e-7,
         1e-8,   1e-9,  1e-10,  1e-11,  1e-12,  1e-13,  1e-14,  1e-15
    ];

    let index = e.abs() as usize;

    if index < 16 {
        if e < 0 {
            NEG_POWERS[index]
        } else {
            POS_POWERS[index]
        }
    } else {
        // powf is more accurate
        10f32.powf(e as f32)
    }
}

static POW10: [f64; 309] =
    [1e000, 1e001, 1e002, 1e003, 1e004, 1e005, 1e006, 1e007, 1e008, 1e009,
     1e010, 1e011, 1e012, 1e013, 1e014, 1e015, 1e016, 1e017, 1e018, 1e019,
     1e020, 1e021, 1e022, 1e023, 1e024, 1e025, 1e026, 1e027, 1e028, 1e029,
     1e030, 1e031, 1e032, 1e033, 1e034, 1e035, 1e036, 1e037, 1e038, 1e039,
     1e040, 1e041, 1e042, 1e043, 1e044, 1e045, 1e046, 1e047, 1e048, 1e049,
     1e050, 1e051, 1e052, 1e053, 1e054, 1e055, 1e056, 1e057, 1e058, 1e059,
     1e060, 1e061, 1e062, 1e063, 1e064, 1e065, 1e066, 1e067, 1e068, 1e069,
     1e070, 1e071, 1e072, 1e073, 1e074, 1e075, 1e076, 1e077, 1e078, 1e079,
     1e080, 1e081, 1e082, 1e083, 1e084, 1e085, 1e086, 1e087, 1e088, 1e089,
     1e090, 1e091, 1e092, 1e093, 1e094, 1e095, 1e096, 1e097, 1e098, 1e099,
     1e100, 1e101, 1e102, 1e103, 1e104, 1e105, 1e106, 1e107, 1e108, 1e109,
     1e110, 1e111, 1e112, 1e113, 1e114, 1e115, 1e116, 1e117, 1e118, 1e119,
     1e120, 1e121, 1e122, 1e123, 1e124, 1e125, 1e126, 1e127, 1e128, 1e129,
     1e130, 1e131, 1e132, 1e133, 1e134, 1e135, 1e136, 1e137, 1e138, 1e139,
     1e140, 1e141, 1e142, 1e143, 1e144, 1e145, 1e146, 1e147, 1e148, 1e149,
     1e150, 1e151, 1e152, 1e153, 1e154, 1e155, 1e156, 1e157, 1e158, 1e159,
     1e160, 1e161, 1e162, 1e163, 1e164, 1e165, 1e166, 1e167, 1e168, 1e169,
     1e170, 1e171, 1e172, 1e173, 1e174, 1e175, 1e176, 1e177, 1e178, 1e179,
     1e180, 1e181, 1e182, 1e183, 1e184, 1e185, 1e186, 1e187, 1e188, 1e189,
     1e190, 1e191, 1e192, 1e193, 1e194, 1e195, 1e196, 1e197, 1e198, 1e199,
     1e200, 1e201, 1e202, 1e203, 1e204, 1e205, 1e206, 1e207, 1e208, 1e209,
     1e210, 1e211, 1e212, 1e213, 1e214, 1e215, 1e216, 1e217, 1e218, 1e219,
     1e220, 1e221, 1e222, 1e223, 1e224, 1e225, 1e226, 1e227, 1e228, 1e229,
     1e230, 1e231, 1e232, 1e233, 1e234, 1e235, 1e236, 1e237, 1e238, 1e239,
     1e240, 1e241, 1e242, 1e243, 1e244, 1e245, 1e246, 1e247, 1e248, 1e249,
     1e250, 1e251, 1e252, 1e253, 1e254, 1e255, 1e256, 1e257, 1e258, 1e259,
     1e260, 1e261, 1e262, 1e263, 1e264, 1e265, 1e266, 1e267, 1e268, 1e269,
     1e270, 1e271, 1e272, 1e273, 1e274, 1e275, 1e276, 1e277, 1e278, 1e279,
     1e280, 1e281, 1e282, 1e283, 1e284, 1e285, 1e286, 1e287, 1e288, 1e289,
     1e290, 1e291, 1e292, 1e293, 1e294, 1e295, 1e296, 1e297, 1e298, 1e299,
     1e300, 1e301, 1e302, 1e303, 1e304, 1e305, 1e306, 1e307, 1e308];

impl From<Number> for f64 {
    fn from(num: Number) -> f64 {
        if num.is_nan() { return f64::NAN; }

        let mut f = num.mantissa as f64;
        let mut exponent = num.exponent;
        loop {
            match POW10.get(exponent.abs() as usize) {
                Some(&pow) => {
                    if exponent >= 0 {
                        f *= pow;
                        if f.is_infinite() {
                            panic!("Number out of range!");
                        }
                    } else {
                        f /= pow;
                    }
                    break;
                }
                None => {
                    if f == 0.0 {
                        break;
                    }
                    if exponent >= 0 {
                        panic!("Number out of range!");
                    }
                    f /= 1e308;
                    exponent += 308;
                }
            }
        }

        if num.is_sign_positive() { f } else { -f }
    }
}

impl From<Number> for f32 {
    fn from(num: Number) -> f32 {
        if num.is_nan() { return f32::NAN; }

        let mut n = num.mantissa as f32;
        let mut e = num.exponent;

        if e < -127 {
            n *= exponent_to_power_f32(e + 127);
            e = -127;
        }

        let f = n * exponent_to_power_f32(e);
        if num.is_sign_positive() { f } else { -f }
    }
}

impl From<f64> for Number {
    fn from(float: f64) -> Number {
        match float.classify() {
            FpCategory::Infinite | FpCategory::Nan => return NAN,
            _ => {}
        }

        if !float.is_sign_positive() {
            let (mantissa, exponent) = grisu2::convert(-float);

            Number::from_parts(false, mantissa, exponent)
        } else {
            let (mantissa, exponent) = grisu2::convert(float);

            Number::from_parts(true, mantissa, exponent)
        }
    }
}

impl From<f32> for Number {
    fn from(float: f32) -> Number {
        match float.classify() {
            FpCategory::Infinite | FpCategory::Nan => return NAN,
            _ => {}
        }

        if !float.is_sign_positive() {
            let (mantissa, exponent) = grisu2::convert(-float as f64);

            Number::from_parts(false, mantissa, exponent)
        } else {
            let (mantissa, exponent) = grisu2::convert(float as f64);

            Number::from_parts(true, mantissa, exponent)
        }
    }
}

impl PartialEq<f64> for Number {
    fn eq(&self, other: &f64) -> bool {
        f64::from(*self) == *other
    }
}

impl PartialEq<f32> for Number {
    fn eq(&self, other: &f32) -> bool {
        f32::from(*self) == *other
    }
}

impl PartialEq<Number> for f64 {
    fn eq(&self, other: &Number) -> bool {
        f64::from(*other) == *self
    }
}

impl PartialEq<Number> for f32 {
    fn eq(&self, other: &Number) -> bool {
        f32::from(*other) == *self
    }
}

macro_rules! impl_unsigned {
    ($( $t:ty ),*) => ($(
        impl From<$t> for Number {
            #[inline]
            fn from(num: $t) -> Number {
                Number {
                    category: POSITIVE,
                    exponent: 0,
                    mantissa: num as u64,
                }
            }
        }

        impl_integer!($t);
    )*)
}


macro_rules! impl_signed {
    ($( $t:ty ),*) => ($(
        impl From<$t> for Number {
            fn from(num: $t) -> Number {
                if num < 0 {
                    Number {
                        category: NEGATIVE,
                        exponent: 0,
                        mantissa: -num as u64,
                    }
                } else {
                    Number {
                        category: POSITIVE,
                        exponent: 0,
                        mantissa: num as u64,
                    }
                }
            }
        }

        impl_integer!($t);
    )*)
}


macro_rules! impl_integer {
    ($t:ty) => {
        impl From<Number> for $t {
            fn from(num: Number) -> $t {
                let (positive, mantissa, exponent) = num.as_parts();

                if exponent <= 0 {
                    if positive {
                        mantissa as $t
                    } else {
                        -(mantissa as i64) as $t
                    }
                } else {
                    // This may overflow, which is fine
                    if positive {
                        (mantissa * 10u64.pow(exponent as u32)) as $t
                    } else {
                        (-(mantissa as i64) * 10i64.pow(exponent as u32)) as $t
                    }
                }
            }
        }

        impl PartialEq<$t> for Number {
            fn eq(&self, other: &$t) -> bool {
                *self == Number::from(*other)
            }
        }

        impl PartialEq<Number> for $t {
            fn eq(&self, other: &Number) -> bool {
                Number::from(*self) == *other
            }
        }
    }
}

impl_signed!(isize, i8, i16, i32, i64);
impl_unsigned!(usize, u8, u16, u32, u64);

impl ops::Neg for Number {
    type Output = Number;

    #[inline]
    fn neg(self) -> Number {
        Number {
            category: self.category ^ POSITIVE,
            exponent: self.exponent,
            mantissa: self.mantissa,
        }
    }
}

// Commented out for now - not doing math ops for 0.10.0
// -----------------------------------------------------
//
// impl ops::Mul for Number {
//     type Output = Number;

//     #[inline]
//     fn mul(self, other: Number) -> Number {
//         // If either is a NaN, return a NaN
//         if (self.category | other.category) & NAN_MASK != 0 {
//             NAN
//         } else {
//             Number {
//                 // If both signs are the same, xoring will produce 0.
//                 // If they are different, xoring will produce 1.
//                 // Xor again with 1 to get a proper proper sign!
//                 // Xor all the things!                              ^ _ ^

//                 category: self.category ^ other.category ^ POSITIVE,
//                 exponent: self.exponent + other.exponent,
//                 mantissa: self.mantissa * other.mantissa,
//             }
//         }
//     }
// }

// impl ops::MulAssign for Number {
//     #[inline]
//     fn mul_assign(&mut self, other: Number) {
//         *self = *self * other;
//     }
// }

#[inline]
fn decimal_power(e: u16) -> u64 {
    static CACHED: [u64; 20] = [
        1,
        10,
        100,
        1000,
        10000,
        100000,
        1000000,
        10000000,
        100000000,
        1000000000,
        10000000000,
        100000000000,
        1000000000000,
        10000000000000,
        100000000000000,
        1000000000000000,
        10000000000000000,
        100000000000000000,
        1000000000000000000,
        10000000000000000000,
    ];

    if e < 20 {
        CACHED[e as usize]
    } else {
        10u64.pow(e as u32)
    }
}
