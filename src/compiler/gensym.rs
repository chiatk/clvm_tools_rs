use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

lazy_static! {
    pub static ref argname_ctr: AtomicUsize = {
        return AtomicUsize::new(0);
    };
}

pub fn gensym(name: Vec<u8>) -> Vec<u8> {
    let count = argname_ctr.fetch_add(1, Ordering::SeqCst);
    let mut result_vec = name;
    let number_value = format!("{}", count+1);
    result_vec.append(&mut "_$_".as_bytes().to_vec());
    result_vec.append(&mut number_value.as_bytes().to_vec());
    return result_vec;
}
