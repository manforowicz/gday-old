const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn str_to_u32(s: &str) -> Option<u32> {
    let s = s
        .trim()
        .to_uppercase()
        .replace(|c| c == 'I' || c == 'L', "1");
    let mut num: u32 = 0;
    for (i, char) in s.as_bytes().iter().rev().enumerate() {
        let Some(digit) = ALPHABET.iter().position(|c| c == char) else {
            return None;
        };

        let Ok(digit) = u32::try_from(digit) else {
            return None;
        };

        let Ok(i) = u32::try_from(i) else { return None };
        let place_value = 32_u32.pow(i);

        let Some(addend) = digit.checked_mul(place_value) else {
            return None;
        };

        let Some(new_num) = num.checked_add(addend) else {
            return None;
        };

        num = new_num;
    }
    Some(num)
}

fn u32_to_str(num: u32) -> String {
    let mut num = num as usize;
    let mut s = Vec::<u8>::new();

    let mut place_value = 32;

    while num != 0 {
        s.push(ALPHABET[num % place_value]);
        num -= num % place_value;
        place_value *= 32;
    }

    s.reverse();

    if s.is_empty() {
        s.push(b'0');
    }

    String::from_utf8(s).unwrap()
}

pub fn to_string(vals: &[u32]) -> String {
    let mut s = String::new();

    for val in vals {
        s.push_str(&u32_to_str(*val));
        s.push('.');
    }
    s
}

pub fn from_string(s: &str) -> Vec<u32> {
    s.trim()
        .split('.')
        .map(|x| {
            str_to_u32(x).unwrap_or_else(|| {
                println!("Invalid code.");
                std::process::exit(1)
            })
        })
        .collect()
}
