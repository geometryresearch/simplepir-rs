use crate::zeroq::ZeroQ;
use crate::matrix::Matrix;
use crate::element::Element;

#[derive(Debug, PartialEq, Clone)]
pub struct Params {
    // Public A matrix
    pub a: Matrix,
    // The integer modulus
    pub q: u64,
    // The plaintext modulus
    pub p: u64,
    // The LWE secret length
    pub n: usize,
    // The number of samples
    pub m: usize,
    // The standard deviation for sampling random elements
    pub std_dev: f64,
}

pub fn simple_params() -> Params {
    let m = 1;
    let n = 512;
    let q = 3329;
    let p = 2;
    let std_dev = 6.4;

    let mut a = Vec::with_capacity(m);
    for _ in 0..m {
        let mut row = Vec::with_capacity(n);
        for _ in 0..n {
            row.push(Element::gen_uniform_rand(q));
        }
        a.push(row);
    }
    let a = Matrix::from(&a);

    Params { a, q, p, n, m, std_dev }
}

fn check_secret_length(params: &Params, secret: &Vec<Element>) {
    // Check that the secret has the correct number of elements
    assert_eq!(secret.len(), params.n);
}

fn check_plaintext_mod(params: &Params, plaintext: &Element) {
    // Check that each element of the plaintext is within range
    assert!(plaintext.uint < params.p);
    assert_eq!(plaintext.q, params.p);
}

fn check_ciphertext_mod(params: &Params, ciphertext: &Element) {
    // Check that the ciphertext is in range
    assert!(ciphertext.uint < params.q);
}

fn check_error_length(params: &Params, error: &Vec<Element>) {
    // Check that the error has the correct number of elements
    assert_eq!(error.len(), params.m);
}

pub fn encrypt(
    params: &Params,
    secret: &Vec<Element>,
    e: &Vec<Element>,
    plaintext: &Element,
) -> Element {
    check_secret_length(params, secret);
    check_plaintext_mod(params, plaintext);
    check_error_length(params, e);
    // TODO: check error range

    // Compute As
    let a_s = params.a.clone().mul_vec(secret);
    
    // Compute b = As + e
    let b = a_s + Matrix::from(&vec![e.clone()]);

    let floor = params.q / params.p;
    let floor = Matrix::from_single(&Element::from(params.q, floor));

    // Convert the plaintext to a matrix with Element mod q instead of p
    let plaintext_as_matrix = Matrix::from_single(&Element::from(params.q, plaintext.uint));

    // Compute the ciphertext As + e + round(q / p) * plaintext
    let c = b + (floor * plaintext_as_matrix);
    
    c[0][0].clone()
}

pub fn decrypt(
    params: &Params,
    secret: &Vec<Element>,
    ciphertext: &Element,
) -> Element {
    check_secret_length(params, secret);
    check_ciphertext_mod(params, ciphertext);
    // Compute As
    let a_s = params.a.clone().mul_vec(secret);

    assert_eq!(ciphertext.q, params.q);
    assert_eq!(a_s[0][0].q, params.q);

    // Compute c - As
    let raw = Matrix::from_single(ciphertext) - a_s;
    //println!("raw * 2 / q: {}", ((raw[0][0].uint * 2) as f64 / params.q as f64).round() as u64 % params.p);

    let x = ((raw[0][0].uint * 2) as f64 / params.q as f64).round() as u64 % params.p;

    Element::from(params.p, x)
}

pub fn gen_random_normal_matrix(
    q: u64,
    std_dev: f64,
    num_rows: usize,
    num_cols: usize,
) -> Matrix {
    let v: Vec<Vec<Element>> = vec![
        vec![Element::new(q); num_rows];
        num_cols
    ];

    let mut matrix = Matrix::from(&v);

    for i in 0..num_cols {
        for j in 0..num_rows {
            matrix[i][j] = Element::gen_normal_rand(q, std_dev);
        }
    }
    matrix
}

pub fn gen_secret(params: &Params) -> Vec<Element> {
    let mut secret = Vec::with_capacity(params.n);
    for _ in 0..params.n {
        secret.push(Element::gen_uniform_rand(params.q));
    }
    secret
}

pub fn gen_error_vec(params: &Params) -> Vec<Element> {
    let sample_space = 6;
    let half_sample_space = sample_space / 2;
    let mut error_vec = Vec::with_capacity(params.m);
    for _ in 0..params.m {
        let rand = Element::gen_uniform_rand(sample_space);
        let mut e = Element::from(params.q, rand.uint);
        e -= Element::from(params.q, half_sample_space);
        error_vec.push(e);
    }
    error_vec
}

pub fn gen_db(db_size: usize) -> Vec<Element> {
    let mut db = Vec::with_capacity(db_size);
    for _ in 0..db_size {
        let val = Element::gen_uniform_rand(2);
        db.push(val);
    }
    db
}

pub fn query(
    params: &Params,
    idx: usize,
    s: &Vec<Element>,
    db_size: usize,
) -> Vec<Element> {
    assert!(idx < db_size);
    let mut query = Vec::with_capacity(db_size);
    for i in 0..db_size {
        let bit;
        if i == idx {
            bit = 1;
        } else {
            bit = 0;
        }
        let e = gen_error_vec(params);
        let enc = encrypt(
            params,
            s,
            &e,
            &Element::from(params.p, bit)
        );
        query.push(enc);
    }
    query
}

pub fn answer(params: &Params, query: &Vec<Element>, db: &Vec<Element>) ->
    (Matrix, Element)
{
    let zero = Element::zero(params.q);
    let row = vec![zero; params.n];
    let cols = vec![row; params.m];
    let mut summed_a = Matrix::from(&cols);
    let mut summed_c = Element::zero(params.q);

    for (i, item) in db.iter().enumerate() {
        if item.uint == 1 {
            summed_a += params.a.clone();
            summed_c += query[i].clone();
        }
    }
    (summed_a, summed_c)
}

#[cfg(test)]
pub mod tests {
    use super::*;

    fn test_pir_impl(
        params: &Params,
        s: &Vec<Element>,
    ) {
        let db_size = 50;
        let db = gen_db(db_size);

        let desired_idx = 24;
        let query = query(&params, desired_idx, &s, db_size);

        let answer = answer(&params, &query, &db);

        // Decrypt the answer
        let mut params_2 = params.clone();
        params_2.a = answer.0;
        let result = decrypt(&params_2, &s, &answer.1);
        assert_eq!(result, db[desired_idx]);
    }

    #[test]
    fn test_pir() {
        let params = simple_params();
        let s = gen_secret(&params);
        for _ in 0..50 {
            test_pir_impl(&params, &s);
        }
    }

    #[test]
    fn test_gen_random_normal_matrix() {
        let num_rows = 9;
        let num_cols = 10;
        let matrix = gen_random_normal_matrix(101u64, 6.4 as f64, num_rows, num_cols);
        assert_eq!(matrix.num_rows(), num_rows);
        assert_eq!(matrix.num_cols(), num_cols);
    }

    fn encrypt_and_decrypt_impl(pu: u64) {
        let params = simple_params();
        let secret = gen_secret(&params);
        let e = gen_error_vec(&params);

        let plaintext = Element::from(params.p, pu);
        let ciphertext = encrypt(&params, &secret, &e, &plaintext);
        assert_eq!(plaintext, decrypt(&params, &secret, &ciphertext));
    }

    #[test]
    fn test_encrypt_and_decrypt() {
        for _ in 0..50 {
            encrypt_and_decrypt_impl(0);
            encrypt_and_decrypt_impl(1);
        }
    }

    fn homomorphic_addition_impl(params: &Params) {
        let secret = gen_secret(&params);
        let e_0 = gen_error_vec(&params);
        let e_1 = gen_error_vec(&params);

        let plaintext_0 = Element::from(params.p, 0);
        let ciphertext_0 = encrypt(&params, &secret, &e_0, &plaintext_0);

        let plaintext_1 = Element::from(params.p, 1);
        let ciphertext_1 = encrypt(&params, &secret, &e_1, &plaintext_1);

        let a_n = params.a.clone() + params.a.clone();
        let mut params = params.clone();
        params.a = a_n;
        let ciphertext_n = ciphertext_0 + ciphertext_1;
        let plaintext_n = plaintext_0 + plaintext_1;
        assert_eq!(plaintext_n, decrypt(&params, &secret, &ciphertext_n));
    }

    #[test]
    fn test_homomorphic_addition() {
        let params = simple_params();
        for _ in 0..50 {
            homomorphic_addition_impl(&params);
        }
    }

    // Homomorphic multiplication isn't needed for bits
}
