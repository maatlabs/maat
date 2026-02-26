let byte: u8 = 255;
let small: i8 = -128;
let medium: i16 = 30000;
let large: i64 = 1000000;

print(byte);
print(small);
print(medium);
print(large);

let hex = 0xCAFEu16;
let bin = 0b10101010u8;
let oct = 0o777u16;

print(hex);
print(bin);
print(oct);

let a = 10i8;
let b = 20i16;
let promoted = a + b;
print(promoted);

let narrow = 200u8;
let wide = narrow as i64;
print(wide);
