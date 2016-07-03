/*
    This is a Rust port of the C implementation found here:
    http://clb.demon.fi/MathGeoLib/nightly/docs/grisu3.c_code.html

    ----------------------- Original head comment: ------------------------

    This file is part of an implementation of the "grisu3" double to string
    conversion algorithm described in the research paper

    "Printing Floating-Point Numbers Quickly And Accurately with Integers"
    by Florian Loitsch, available at
    http://www.cs.tufts.edu/~nr/cs257/archive/florian-loitsch/printf.pdf
*/

use std::mem;
use std::io::Write;
use std::slice;

const D64_SIGN: u64         = 0x8000000000000000;
const D64_EXP_MASK: u64     = 0x7FF0000000000000;
const D64_FRACT_MASK: u64   = 0x000FFFFFFFFFFFFF;
const D64_IMPLICIT_ONE: u64 = 0x0010000000000000;
const D64_EXP_POS: u64      = 52;
const D64_EXP_BIAS: i32     = 1075;
const DIYFP_FRACT_SIZE: i32 = 64;
const D_1_LOG2_10: f64      = 0.30102999566398114;
const MIN_TARGET_EXP: i32   = -60;
const MASK32: u64           = 0xFFFFFFFF;
const MIN_CACHED_EXP: i32   = -348;
const CACHED_EXP_STEP: i32  = 8;

/*
#define CAST_U64(d) (*(uint64_t*)&d)
 */
#[inline(always)]
fn cast_u64(d: f64) -> u64 {
    unsafe { mem::transmute(d) }
}

macro_rules! ptr_inc_set {
    ($ptr:ident, $value:expr) => (unsafe {
        *$ptr = $value;
        $ptr = $ptr.offset(1);
    })
}

/*
#define MIN(x,y) ((x) <= (y) ? (x) : (y))
#define MAX(x,y) ((x) >= (y) ? (x) : (y))
*/
macro_rules! min {
    ($x:expr, $y:expr) => (if $x <= $y { $x } else { $y })
}
macro_rules! max {
    ($x:expr, $y:expr) => (if $x >= $y { $x } else { $y })
}

/*
typedef struct diy_fp
{
        uint64_t f;
        int e;
} diy_fp;
 */
#[derive(Copy, Clone)]
struct DiyFp {
    pub f: u64,
    pub e: i32,
}

/*
typedef struct power
{
        uint64_t fract;
        int16_t b_exp, d_exp;
} power;
 */
#[derive(Copy, Clone)]
struct Power {
    pub fract: u64,
    pub b_exp: i32,
    pub d_exp: i32,
}

/*
static const power pow_cache[] =
{
        { 0xfa8fd5a0081c0288ULL, -1220, -348 },
        ...
        { 0xaf87023b9bf0ee6bULL,  1066,  340 }
};
 */
static POW_CACHE: [Power; 87] = [
        Power { fract: 0xfa8fd5a0081c0288, b_exp: -1220, d_exp: -348 },
        Power { fract: 0xbaaee17fa23ebf76, b_exp: -1193, d_exp: -340 },
        Power { fract: 0x8b16fb203055ac76, b_exp: -1166, d_exp: -332 },
        Power { fract: 0xcf42894a5dce35ea, b_exp: -1140, d_exp: -324 },
        Power { fract: 0x9a6bb0aa55653b2d, b_exp: -1113, d_exp: -316 },
        Power { fract: 0xe61acf033d1a45df, b_exp: -1087, d_exp: -308 },
        Power { fract: 0xab70fe17c79ac6ca, b_exp: -1060, d_exp: -300 },
        Power { fract: 0xff77b1fcbebcdc4f, b_exp: -1034, d_exp: -292 },
        Power { fract: 0xbe5691ef416bd60c, b_exp: -1007, d_exp: -284 },
        Power { fract: 0x8dd01fad907ffc3c, b_exp:  -980, d_exp: -276 },
        Power { fract: 0xd3515c2831559a83, b_exp:  -954, d_exp: -268 },
        Power { fract: 0x9d71ac8fada6c9b5, b_exp:  -927, d_exp: -260 },
        Power { fract: 0xea9c227723ee8bcb, b_exp:  -901, d_exp: -252 },
        Power { fract: 0xaecc49914078536d, b_exp:  -874, d_exp: -244 },
        Power { fract: 0x823c12795db6ce57, b_exp:  -847, d_exp: -236 },
        Power { fract: 0xc21094364dfb5637, b_exp:  -821, d_exp: -228 },
        Power { fract: 0x9096ea6f3848984f, b_exp:  -794, d_exp: -220 },
        Power { fract: 0xd77485cb25823ac7, b_exp:  -768, d_exp: -212 },
        Power { fract: 0xa086cfcd97bf97f4, b_exp:  -741, d_exp: -204 },
        Power { fract: 0xef340a98172aace5, b_exp:  -715, d_exp: -196 },
        Power { fract: 0xb23867fb2a35b28e, b_exp:  -688, d_exp: -188 },
        Power { fract: 0x84c8d4dfd2c63f3b, b_exp:  -661, d_exp: -180 },
        Power { fract: 0xc5dd44271ad3cdba, b_exp:  -635, d_exp: -172 },
        Power { fract: 0x936b9fcebb25c996, b_exp:  -608, d_exp: -164 },
        Power { fract: 0xdbac6c247d62a584, b_exp:  -582, d_exp: -156 },
        Power { fract: 0xa3ab66580d5fdaf6, b_exp:  -555, d_exp: -148 },
        Power { fract: 0xf3e2f893dec3f126, b_exp:  -529, d_exp: -140 },
        Power { fract: 0xb5b5ada8aaff80b8, b_exp:  -502, d_exp: -132 },
        Power { fract: 0x87625f056c7c4a8b, b_exp:  -475, d_exp: -124 },
        Power { fract: 0xc9bcff6034c13053, b_exp:  -449, d_exp: -116 },
        Power { fract: 0x964e858c91ba2655, b_exp:  -422, d_exp: -108 },
        Power { fract: 0xdff9772470297ebd, b_exp:  -396, d_exp: -100 },
        Power { fract: 0xa6dfbd9fb8e5b88f, b_exp:  -369, d_exp:  -92 },
        Power { fract: 0xf8a95fcf88747d94, b_exp:  -343, d_exp:  -84 },
        Power { fract: 0xb94470938fa89bcf, b_exp:  -316, d_exp:  -76 },
        Power { fract: 0x8a08f0f8bf0f156b, b_exp:  -289, d_exp:  -68 },
        Power { fract: 0xcdb02555653131b6, b_exp:  -263, d_exp:  -60 },
        Power { fract: 0x993fe2c6d07b7fac, b_exp:  -236, d_exp:  -52 },
        Power { fract: 0xe45c10c42a2b3b06, b_exp:  -210, d_exp:  -44 },
        Power { fract: 0xaa242499697392d3, b_exp:  -183, d_exp:  -36 },
        Power { fract: 0xfd87b5f28300ca0e, b_exp:  -157, d_exp:  -28 },
        Power { fract: 0xbce5086492111aeb, b_exp:  -130, d_exp:  -20 },
        Power { fract: 0x8cbccc096f5088cc, b_exp:  -103, d_exp:  -12 },
        Power { fract: 0xd1b71758e219652c, b_exp:   -77, d_exp:   -4 },
        Power { fract: 0x9c40000000000000, b_exp:   -50, d_exp:    4 },
        Power { fract: 0xe8d4a51000000000, b_exp:   -24, d_exp:   12 },
        Power { fract: 0xad78ebc5ac620000, b_exp:     3, d_exp:   20 },
        Power { fract: 0x813f3978f8940984, b_exp:    30, d_exp:   28 },
        Power { fract: 0xc097ce7bc90715b3, b_exp:    56, d_exp:   36 },
        Power { fract: 0x8f7e32ce7bea5c70, b_exp:    83, d_exp:   44 },
        Power { fract: 0xd5d238a4abe98068, b_exp:   109, d_exp:   52 },
        Power { fract: 0x9f4f2726179a2245, b_exp:   136, d_exp:   60 },
        Power { fract: 0xed63a231d4c4fb27, b_exp:   162, d_exp:   68 },
        Power { fract: 0xb0de65388cc8ada8, b_exp:   189, d_exp:   76 },
        Power { fract: 0x83c7088e1aab65db, b_exp:   216, d_exp:   84 },
        Power { fract: 0xc45d1df942711d9a, b_exp:   242, d_exp:   92 },
        Power { fract: 0x924d692ca61be758, b_exp:   269, d_exp:  100 },
        Power { fract: 0xda01ee641a708dea, b_exp:   295, d_exp:  108 },
        Power { fract: 0xa26da3999aef774a, b_exp:   322, d_exp:  116 },
        Power { fract: 0xf209787bb47d6b85, b_exp:   348, d_exp:  124 },
        Power { fract: 0xb454e4a179dd1877, b_exp:   375, d_exp:  132 },
        Power { fract: 0x865b86925b9bc5c2, b_exp:   402, d_exp:  140 },
        Power { fract: 0xc83553c5c8965d3d, b_exp:   428, d_exp:  148 },
        Power { fract: 0x952ab45cfa97a0b3, b_exp:   455, d_exp:  156 },
        Power { fract: 0xde469fbd99a05fe3, b_exp:   481, d_exp:  164 },
        Power { fract: 0xa59bc234db398c25, b_exp:   508, d_exp:  172 },
        Power { fract: 0xf6c69a72a3989f5c, b_exp:   534, d_exp:  180 },
        Power { fract: 0xb7dcbf5354e9bece, b_exp:   561, d_exp:  188 },
        Power { fract: 0x88fcf317f22241e2, b_exp:   588, d_exp:  196 },
        Power { fract: 0xcc20ce9bd35c78a5, b_exp:   614, d_exp:  204 },
        Power { fract: 0x98165af37b2153df, b_exp:   641, d_exp:  212 },
        Power { fract: 0xe2a0b5dc971f303a, b_exp:   667, d_exp:  220 },
        Power { fract: 0xa8d9d1535ce3b396, b_exp:   694, d_exp:  228 },
        Power { fract: 0xfb9b7cd9a4a7443c, b_exp:   720, d_exp:  236 },
        Power { fract: 0xbb764c4ca7a44410, b_exp:   747, d_exp:  244 },
        Power { fract: 0x8bab8eefb6409c1a, b_exp:   774, d_exp:  252 },
        Power { fract: 0xd01fef10a657842c, b_exp:   800, d_exp:  260 },
        Power { fract: 0x9b10a4e5e9913129, b_exp:   827, d_exp:  268 },
        Power { fract: 0xe7109bfba19c0c9d, b_exp:   853, d_exp:  276 },
        Power { fract: 0xac2820d9623bf429, b_exp:   880, d_exp:  284 },
        Power { fract: 0x80444b5e7aa7cf85, b_exp:   907, d_exp:  292 },
        Power { fract: 0xbf21e44003acdd2d, b_exp:   933, d_exp:  300 },
        Power { fract: 0x8e679c2f5e44ff8f, b_exp:   960, d_exp:  308 },
        Power { fract: 0xd433179d9c8cb841, b_exp:   986, d_exp:  316 },
        Power { fract: 0x9e19db92b4e31ba9, b_exp:  1013, d_exp:  324 },
        Power { fract: 0xeb96bf6ebadf77d9, b_exp:  1039, d_exp:  332 },
        Power { fract: 0xaf87023b9bf0ee6b, b_exp:  1066, d_exp:  340 }
];

/*
static int cached_pow(int exp, diy_fp *p)
{
        int k = (int)ceil((exp+DIYFP_FRACT_SIZE-1) * D_1_LOG2_10);
        int i = (k-MIN_CACHED_EXP-1) / CACHED_EXP_STEP + 1;
        p->f = pow_cache[i].fract;
        p->e = pow_cache[i].b_exp;
        return pow_cache[i].d_exp;
}
 */
#[inline(always)]
fn cached_pow(exp: i32, p: &mut DiyFp) -> i32 {
    let k = (((exp + DIYFP_FRACT_SIZE - 1) as f64) * D_1_LOG2_10 ).ceil() as i32;
    let i = (k - MIN_CACHED_EXP - 1) / CACHED_EXP_STEP + 1;
    let power = POW_CACHE[i as usize];
    p.f = power.fract;
    p.e = power.b_exp;
    power.d_exp
}

/*
static diy_fp minus(diy_fp x, diy_fp y)
{
        diy_fp d; d.f = x.f - y.f; d.e = x.e;
        assert(x.e == y.e && x.f >= y.f);
        return d;
}
 */
#[inline(always)]
fn minus(x: &DiyFp, y: &DiyFp) -> DiyFp {
    assert!(x.e == y.e && x.f >= y.f);

    DiyFp {
        f: x.f - y.f,
        e: x.e
    }
}

/*
static diy_fp multiply(diy_fp x, diy_fp y)
{
        uint64_t a, b, c, d, ac, bc, ad, bd, tmp;
        diy_fp r;
        a = x.f >> 32; b = x.f & MASK32;
        c = y.f >> 32; d = y.f & MASK32;
        ac = a*c; bc = b*c;
        ad = a*d; bd = b*d;
        tmp = (bd >> 32) + (ad & MASK32) + (bc & MASK32);
        tmp += 1U << 31; // round
        r.f = ac + (ad >> 32) + (bc >> 32) + (tmp >> 32);
        r.e = x.e + y.e + 64;
        return r;
}
 */
fn multiply(x: &DiyFp, y: &DiyFp) -> DiyFp {
    let a = x.f >> 32; let b = x.f & MASK32;
    let c = y.f >> 32; let d = y.f & MASK32;
    let ac = a * c; let bc = b as u64 * c as u64;
    let ad = a * d; let bd = b as u64 * d as u64;
    let mut tmp = (bd >> 32) + (ad & MASK32) + (bc & MASK32);
    tmp += 1 << 31; // round

    DiyFp {
        f: (ac as u64) + ((ad as u64) >> 32) + ((bc as u64) >> 32) + ((tmp as u64) >> 32),
        e: x.e + y.e + 64,
    }
}

/*
static diy_fp normalize_diy_fp(diy_fp n)
{
        assert(n.f != 0);
        while(!(n.f & 0xFFC0000000000000ULL)) { n.f <<= 10; n.e -= 10; }
        while(!(n.f & D64_SIGN)) { n.f <<= 1; --n.e; }
        return n;
}
 */
fn normalize_diy_fp(mut n: DiyFp) -> DiyFp {
    assert!(n.f != 0);
    while (n.f & 0xFFC0000000000000) == 0 {
        n.f <<= 10;
        n.e -= 10;
    }
    while (n.f & D64_SIGN) == 0 {
        n.f <<= 1;
        n.e -= 1;
    }

    n
}

/*
static diy_fp double2diy_fp(double d)
{
        diy_fp fp;
        uint64_t u64 = CAST_U64(d);
        if (!(u64 & D64_EXP_MASK)) { fp.f = u64 & D64_FRACT_MASK; fp.e = 1 - D64_EXP_BIAS; }
        else { fp.f = (u64 & D64_FRACT_MASK) + D64_IMPLICIT_ONE; fp.e = (int)((u64 & D64_EXP_MASK) >> D64_EXP_POS) - D64_EXP_BIAS; }
        return fp;
}
 */
fn double2diy_fp(d: f64) -> DiyFp {
    let u = cast_u64(d);
    if (u & D64_EXP_MASK) == 0 {
        DiyFp {
            f: u & D64_FRACT_MASK,
            e: 1 - D64_EXP_BIAS,
        }
    } else {
        DiyFp {
            f: (u & D64_FRACT_MASK) + D64_IMPLICIT_ONE,
            e: (((u & D64_EXP_MASK) >> D64_EXP_POS) as i32) - D64_EXP_BIAS,
        }
    }
}

// pow10_cache[i] = 10^(i-1)
/* static const unsigned int pow10_cache[] = { 0, 1, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000 }; */
static POW10_CACHE: [u32; 11] = [ 0, 1, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000 ];

/*
static int largest_pow10(uint32_t n, int n_bits, uint32_t *power)
{
        int guess = ((n_bits + 1) * 1233 >> 12) + 1/*skip first entry*/;
        if (n < pow10_cache[guess]) --guess; // We don't have any guarantees that 2^n_bits <= n.
        *power = pow10_cache[guess];
        return guess;
}
*/
fn largest_pow10(n: u32, n_bits: i32, power: &mut u32) -> i32 {
    let mut guess = (((n_bits + 1) * 1233 >> 12) + 1/*skip first entry*/) as usize;
    let mut pow = POW10_CACHE[guess];
    if n < pow {
        guess -= 1;
        pow = POW10_CACHE[guess];
    }
    *power = pow;

    guess as i32
}

/*
static int round_weed(char *buffer, int len, uint64_t wp_W, uint64_t delta, uint64_t rest, uint64_t ten_kappa, uint64_t ulp)
{
        uint64_t wp_Wup = wp_W - ulp;
        uint64_t wp_Wdown = wp_W + ulp;
        while(rest < wp_Wup && delta - rest >= ten_kappa
                && (rest + ten_kappa < wp_Wup || wp_Wup - rest >= rest + ten_kappa - wp_Wup))
        {
                --buffer[len-1];
                rest += ten_kappa;
        }
        if (rest < wp_Wdown && delta - rest >= ten_kappa
                && (rest + ten_kappa < wp_Wdown || wp_Wdown - rest > rest + ten_kappa - wp_Wdown))
                return 0;

        return 2*ulp <= rest && rest <= delta - 4*ulp;
}
 */
fn round_weed(buffer: *mut u8, len: isize, wp_W: u64, delta: u64, mut rest: u64, ten_kappa: u64, ulp: u64) -> i32 {
        let wp_Wup = wp_W - ulp;
        let wp_Wdown = wp_W + ulp;

        while rest < wp_Wup && delta - rest >= ten_kappa
        && (rest + ten_kappa < wp_Wup || wp_Wup - rest >= rest + ten_kappa - wp_Wup) {
            unsafe { *buffer.offset((len as isize) - 1) -= 1 };
            rest += ten_kappa;
        }

        if rest < wp_Wdown && delta - rest >= ten_kappa
        && (rest + ten_kappa < wp_Wdown || wp_Wdown - rest > rest + ten_kappa - wp_Wdown) {
            return 0;
        }

        (2 * ulp <= rest && rest <= delta - 4 * ulp) as i32
}

/*
static int digit_gen(diy_fp low, diy_fp w, diy_fp high, char *buffer, int *length, int *kappa)
{
        uint64_t unit = 1;
        diy_fp too_low = { low.f - unit, low.e };
        diy_fp too_high = { high.f + unit, high.e };
        diy_fp unsafe_interval = minus(too_high, too_low);
        diy_fp one = { 1ULL << -w.e, w.e };
        uint32_t p1 = (uint32_t)(too_high.f >> -one.e);
        uint64_t p2 = too_high.f & (one.f - 1);
        uint32_t div;
        *kappa = largest_pow10(p1, DIYFP_FRACT_SIZE + one.e, &div);
        *length = 0;

        while(*kappa > 0)
        {
                uint64_t rest;
                int digit = p1 / div;
                buffer[*length] = (char)('0' + digit);
                ++*length;
                p1 %= div;
                --*kappa;
                rest = ((uint64_t)p1 << -one.e) + p2;
                if (rest < unsafe_interval.f) return round_weed(buffer, *length, minus(too_high, w).f, unsafe_interval.f, rest, (uint64_t)div << -one.e, unit);
                div /= 10;
        }

        for(;;)
        {
                int digit;
                p2 *= 10;
                unit *= 10;
                unsafe_interval.f *= 10;
                // Integer division by one.
                digit = (int)(p2 >> -one.e);
                buffer[*length] = (char)('0' + digit);
                ++*length;
                p2 &= one.f - 1;  // Modulo by one.
                --*kappa;
                if (p2 < unsafe_interval.f) return round_weed(buffer, *length, minus(too_high, w).f * unit, unsafe_interval.f, p2, one.f, unit);
        }
}
*/
fn digit_gen(low: DiyFp, w: DiyFp, high: DiyFp, buffer: *mut u8, length: &mut isize, kappa: &mut i32) -> i32 {
    let mut unit = 1u64;
    let too_low = DiyFp {
        f: low.f - unit,
        e: low.e
    };
    let too_high = DiyFp {
        f: high.f + unit,
        e: high.e
    };
    let mut unsafe_interval = minus(&too_high, &too_low);
    let one = DiyFp {
        f: 1 << (-w.e as u64),
        e: w.e
    };
    let mut p1 = (too_high.f >> -one.e as u64) as u32;
    let mut p2 = too_high.f & (one.f - 1);
    let mut div: u32 = unsafe { mem::uninitialized() };
    *kappa = largest_pow10(p1, DIYFP_FRACT_SIZE + one.e, &mut div);
    *length = 0;

    while *kappa > 0 {
        let digit = (p1 / div) as u8;
        unsafe { *buffer.offset(*length as isize) = b'0' + digit };
        *length += 1;
        p1 %= div;
        *kappa -= 1;
        let rest = ((p1 as u64) << (-one.e as u64)) + p2;
        if rest < unsafe_interval.f {
            return round_weed(buffer, *length, minus(&too_high, &w).f, unsafe_interval.f, rest, (div as u64) << ((-one.e) as u64), unit);
        }
        div /= 10;
    }

    loop {
        p2 = (p2 << 3) + (p2 << 1); // FIXME: use shifts
        unit = (unit << 3) + (unit << 1);
        unsafe_interval.f = (unsafe_interval.f << 3) + (unsafe_interval.f << 1);

        // Integer division by one.
        let digit = (p2 >> ((-one.e) as u64)) as u8;
        unsafe { *buffer.offset(*length as isize) = b'0' + digit };
        *length += 1;
        p2 &= one.f - 1; // Modulo by one.
        *kappa -= 1;
        if p2 < unsafe_interval.f {
            return round_weed(buffer, *length, minus(&too_high, &w).f * unit, unsafe_interval.f, p2, one.f, unit);
        }
    }
}

/*
static int grisu3(double v, char *buffer, int *length, int *d_exp)
{
        int mk, kappa, success;
        diy_fp dfp = double2diy_fp(v);
        diy_fp w = normalize_diy_fp(dfp);

        // normalize boundaries
        diy_fp t = { (dfp.f << 1) + 1, dfp.e - 1 };
        diy_fp b_plus = normalize_diy_fp(t);
        diy_fp b_minus;
        diy_fp c_mk; // Cached power of ten: 10^-k
        uint64_t u64 = CAST_U64(v);
        assert(v > 0 && v <= 1.7976931348623157e308); // Grisu only handles strictly positive finite numbers.
        if (!(u64 & D64_FRACT_MASK) && (u64 & D64_EXP_MASK) != 0) { b_minus.f = (dfp.f << 2) - 1; b_minus.e =  dfp.e - 2;} // lower boundary is closer?
        else { b_minus.f = (dfp.f << 1) - 1; b_minus.e = dfp.e - 1; }
        b_minus.f = b_minus.f << (b_minus.e - b_plus.e);
        b_minus.e = b_plus.e;

        mk = cached_pow(MIN_TARGET_EXP - DIYFP_FRACT_SIZE - w.e, &c_mk);

        w = multiply(w, c_mk);
        b_minus = multiply(b_minus, c_mk);
        b_plus  = multiply(b_plus,  c_mk);

        success = digit_gen(b_minus, w, b_plus, buffer, length, &kappa);
        *d_exp = kappa - mk;
        return success;
}
*/
fn grisu3(v: f64, buffer: *mut u8, length: &mut isize, d_exp: &mut i32) -> i32 {
    let dfp = double2diy_fp(v);
    let mut w = normalize_diy_fp(dfp);

    // normalize boundaries
    let t = DiyFp {
        f: (dfp.f << 1) + 1,
        e: dfp.e -1
    };
    let mut b_plus = normalize_diy_fp(t);

    let u = cast_u64(v);
    assert!(v > 0.0 && v <= 1.7976931348623157e308); // Grisu only handles strictly positive finite numbers.

    let mut b_minus = if (u & D64_FRACT_MASK) == 0 && (u & D64_EXP_MASK) != 0 {
        DiyFp {
            f: (dfp.f << 2) - 1,
            e: dfp.e - 2 // lower boundary is closer?
        }
    } else {
        DiyFp {
            f: (dfp.f << 1) - 1,
            e: dfp.e - 1
        }
    };

    b_minus.f = b_minus.f << ((b_minus.e - b_plus.e) as u64);
    b_minus.e = b_plus.e;

    let mut c_mk: DiyFp = unsafe { mem::uninitialized() }; // Cached power of ten: 10^-k
    let mk = cached_pow(MIN_TARGET_EXP - DIYFP_FRACT_SIZE - w.e, &mut c_mk);

    w = multiply(&w, &c_mk);
    b_minus = multiply(&b_minus, &c_mk);
    b_plus  = multiply(&b_plus,  &c_mk);

    let mut kappa: i32 = unsafe { mem::uninitialized() };
    let success = digit_gen(b_minus, w, b_plus, buffer, length, &mut kappa);
    *d_exp = kappa - mk;

    success
}

// Returns length of u when written out as a string, u must be in the range of [-9999, 9999].
/*
static int exp_len(int u)
{
        if (u > 0) return u >= 1000 ? 4 : (u >= 100 ? 3 : (u >= 10 ? 2 : 1));
        else if (u < 0) return u <= -1000 ? 5 : (u <= -100 ? 4 : (u <= -10 ? 3 : 2));
        else return 1;
}
*/
fn exp_len(u: i32) -> i32 {
    if u > 0 {
        if      u >=  1000 { 4 }
        else if u >=   100 { 3 }
        else if u >=    10 { 2 }
        else               { 1 }
    } else if u < 0 {
        if      u <= -1000 { 5 }
        else if u <=  -100 { 4 }
        else if u <=   -10 { 3 }
        else               { 2 }
    } else {
        1
    }
}

/*
static int i_to_str(int val, char *str)
{
        int len, i;
        char *s;
        char *begin = str;
        if (val < 0) { *str++ = '-'; val = -val; }
        s = str;

        for(;;)
        {
                int ni = val / 10;
                int digit = val - ni*10;
                *s++ = (char)('0' + digit);
                if (ni == 0)
                        break;
                val = ni;
        }
        *s = '\0';
        len = (int)(s - str);
        for(i = 0; i < len/2; ++i)
        {
                char ch = str[i];
                str[i] = str[len-1-i];
                str[len-1-i] = ch;
        }

        return (int)(s - begin);
}
*/
fn i_to_str(mut val: i32, mut str: *mut u8) -> isize {
    // char *s;
    let begin = str;
    if val < 0 {
        ptr_inc_set!(str, b'-');
        val = -val;
    }
    let mut s = str;

    loop {
        let ni = val / 10;
        let digit = (val - (ni << 3 + ni << 1)) as u8;
        ptr_inc_set!(s, b'0' + digit);
        if ni == 0 {
            break;
        }
        val = ni;
    }
    unsafe { *s = b'\0' };
    let len = (s as isize) - (str as isize);
    let mut i = 0;
    while i < len/2 {
        let ch = unsafe { *str.offset(i) };
        unsafe { *str.offset(i) = *str.offset(len - 1 - i) };
        unsafe { *str.offset(len-1-i) = ch }
        i += 1;
    }

    (s as isize) - (begin as isize)
}

/*
int dtoa_grisu3(double v, char *dst)
{
        int d_exp, len, success, decimals, i;
        uint64_t u64 = CAST_U64(v);
        char *s2 = dst;
        assert(dst);

        // Prehandle NaNs
        if ((u64 << 1) > 0xFFE0000000000000ULL) return sprintf(dst, "NaN(%08X%08X)", (uint32_t)(u64 >> 32), (uint32_t)u64);
        // Prehandle negative values.
        if ((u64 & D64_SIGN) != 0) { *s2++ = '-'; v = -v; u64 ^= D64_SIGN; }
        // Prehandle zero.
        if (!u64) { *s2++ = '0'; *s2 = '\0'; return (int)(s2 - dst); }
        // Prehandle infinity.
        if (u64 == D64_EXP_MASK) { *s2++ = 'i'; *s2++ = 'n'; *s2++ = 'f'; *s2 = '\0'; return (int)(s2 - dst); }

        success = grisu3(v, s2, &len, &d_exp);
        // If grisu3 was not able to convert the number to a string, then use old sprintf (suboptimal).
        if (!success) return sprintf(s2, "%.17g", v) + (int)(s2 - dst);

        decimals = MIN(-d_exp, MAX(1, len-1));
        if (d_exp < 0 && (len >= -d_exp || exp_len(d_exp+decimals)+1 <= exp_len(d_exp))) // Add decimal point?
        {
                for(i = 0; i < decimals; ++i) s2[len-i] = s2[len-i-1];
                s2[len++ - decimals] = '.';
                d_exp += decimals;
                // Need scientific notation as well?
                if (d_exp != 0) { s2[len++] = 'e'; len += i_to_str(d_exp, s2+len); }
        }
        else if (d_exp < 0 && d_exp >= -3) // Add decimal point for numbers of form 0.000x where it's shorter?
        {
                for(i = 0; i < len; ++i) s2[len-d_exp-1-i] = s2[len-i-1];
                s2[0] = '.';
                for(i = 1; i < -d_exp; ++i) s2[i] = '0';
                len += -d_exp;
        }
        // Add scientific notation?
        else if (d_exp < 0 || d_exp > 2) { s2[len++] = 'e'; len += i_to_str(d_exp, s2+len); }
        // Add zeroes instead of scientific notation?
        else if (d_exp > 0) { while(d_exp-- > 0) s2[len++] = '0'; }
        s2[len] = '\0'; // grisu3 doesn't null terminate, so ensure termination.
        return (int)(s2+len-dst);
}
 */
pub fn write<W: Write>(writer: &mut W, mut v: f64) {
    // int d_exp, len, success, decimals, i;
    let mut u = cast_u64(v);

    let mut buf: [u8; 32] = unsafe { mem::uninitialized() };
    let mut dst = buf.as_mut_ptr();
    let mut s2 = dst;



    // Prehandle NaNs
    if (u << 1) > 0xFFE0000000000000 {
        panic!("NAN!");
    }

    // Prehandle negative values.
    if (u & D64_SIGN) != 0 {
        ptr_inc_set!(s2, b'-');
        v = -v;
        u ^= D64_SIGN;
    }

    // Prehandle zero.
    if u == 0 {
        ptr_inc_set!(s2, b'0');
        let length = (s2 as usize) - (dst as usize);
        writer.write_all(unsafe { slice::from_raw_parts(dst, length) });
        return;
    }

    // Prehandle infinity.
    if u == D64_EXP_MASK {
        panic!("INF!");
    }

    let mut len: isize = unsafe { mem::uninitialized() };
    let mut d_exp: i32 = unsafe { mem::uninitialized() };
    let success = grisu3(v, s2, &mut len, &mut d_exp);

    // If grisu3 was not able to convert the number to a string, then use old sprintf (suboptimal).
    if success == 0 {
        panic!("GRISU CANNOT DO!")
    }

    let decimals = min!(-d_exp, max!(1, (len as i32)-1));
    if d_exp < 0 && (len as i32 >= -d_exp || exp_len(d_exp + decimals) + 1 <= exp_len(d_exp)) {
        // Add decimal point?
        let mut i = 0;
        while i < decimals {
            unsafe { *s2.offset(len - (i as isize)) = *s2.offset((len as isize) - (i-1) as isize) };
            i += 1;
        }
        unsafe { *s2.offset(len - (decimals as isize)) = b'.'; }
        len += 1;
        d_exp += decimals;
        // Need scientific notation as well?
        if d_exp != 0 {
            unsafe { *s2.offset(len) = b'e' };
            len += 1 + i_to_str(d_exp, unsafe { s2.offset(len as isize) });
        }
    } else if d_exp < 0 && d_exp >= -3 { // Add decimal point for numbers of form 0.000x where it's shorter?
        let mut i = 0;
        while i < len {
            unsafe { *s2.offset(len - (d_exp as isize) - 1 - i) = *s2.offset(len - i - 1) };
            i += 1;
        }
        unsafe { *s2 = b'.' };
        i = 1;
        let cap = -d_exp as isize;
        while i < cap {
            unsafe { *s2.offset(i) = b'0' };
            i += 1;
        }
        len += cap;
    } else if d_exp < 0 || d_exp > 2 {
        // Add scientific notation?
        unsafe { *s2.offset(len) = b'e' };
        len += 1;
        len += i_to_str(d_exp, unsafe { s2.offset(len) });
    } else if d_exp > 0 {
        // Add zeroes instead of scientific notation?
        while d_exp > 0 {
            unsafe { *s2.offset(len) = b'0' };
            len += 1;
            d_exp -= 1;
        }
    }
    unsafe { *s2.offset(len) = b'0' }; // grisu3 doesn't null terminate, so ensure termination.

    let length = (s2 as usize) + (len as usize) - (dst as usize);
    writer.write_all(unsafe { slice::from_raw_parts(dst, length) });
}
