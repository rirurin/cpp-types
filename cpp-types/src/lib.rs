#[cfg(feature = "clang")]
pub mod clang {
    pub mod string;
    pub mod vector;
}
#[cfg(feature = "gcc")]
pub mod gcc {
    pub mod string;
    pub mod vector;
}
pub mod generic {
    pub mod string;
    pub mod vector;
}
#[cfg(feature = "msvc")]
pub mod msvc {
    pub mod function;
    pub mod hash;
    pub mod list;
    pub mod mutex;
    pub mod optional;
    pub mod shared_ptr;
    pub mod tree;
    pub mod string;
    pub mod type_info;
    pub mod unordered;
    pub mod vector;
}
