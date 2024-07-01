pub use serde_json::json as impl_json_inner;

#[macro_export]
macro_rules! json {
    ($ty:ty { $($items:tt)* }) => {
        {
            <$ty as serde::Deserialize>::deserialize($crate::helpers::impl_json_inner!({$($items)*})).unwrap()
        }
    };
}

pub fn from_json_string<'de, T: serde::Deserialize<'de>>(str: &'de str) -> T {
    serde_json::from_str(str).unwrap()
}
