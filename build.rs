fn main() {
    if std::env::var("cargo_feature_c_smt_impl".to_uppercase()) == Ok(String::from("1")) {
        cc::Build::new()
            .file("deps/ckb_smt.c")
            .static_flag(true)
            .flag("-O3")
            .flag("-fvisibility=hidden")
            .flag("-fdata-sections")
            .flag("-ffunction-sections")
            .include("deps/ckb-c-stdlib")
            .flag("-Wall")
            .flag("-Werror")
            .flag("-Wno-unused-parameter")
            .flag("-Wno-nonnull")
            .define("__SHARED_LIBRARY__", None)
            .define("CKB_STDLIB_NO_SYSCALL_IMPL", None)
            .compile("dl-c-impl");
    }
}
