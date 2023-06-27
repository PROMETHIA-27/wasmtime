//! Measure how quickly cranelift can compile and run 2 factorial functions, separately.
//! One function is defined recursively, one is defined iteratively. It also benches two equivalent
//! rust factorial functions, to compare.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::immediates::Imm64;
use cranelift_codegen::ir::{
    types, AbiParam, ExternalName, FuncRef, Function, InstructionData, Opcode, Signature,
    UserExternalName, UserFuncName,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::Module;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

mod clifp;

const CLIF_REC_FAC: &str = "
function %rec_fac(i32) -> i32 system_v {
    fn0 = %rec_fac(i32) -> i32 system_v

block0(v0: i32):
    brif v0, block1(v0), block2

block1(v1: i32):
    v2 = iadd_imm v1, -1
    v3 = call fn0(v2)
    v4 = imul v1, v3
    return v4

block2:
    v5 = iconst.i32 1
    return v5
}
";

const CLIF_ITER_FAC: &str = "
function %iter_fac(i32) -> i32 system_v {

;      n
block0(v0: i32):
    v1 = iconst.i32 1
    brif v0, block1(v0, v0), block1(v0, v1)

;      n        acc
block1(v2: i32, v3: i32): ; while n > 1 
    v4 = iconst.i32 1
    v5 = icmp ugt v2, v4 ; n > 1
    brif v5, block2(v2, v3), block3(v3)

;      n        acc
block2(v6: i32, v7: i32): ; {...}
    v8 = iadd_imm v6, -1 ; n - 1
    v9 = imul v7, v8 ; acc
    jump block1(v8, v9)

;      acc
block3(v10: i32):
    return v10

}
";

fn rec_factorial(n: u32) -> u32 {
    if n != 0 {
        rec_factorial(n - 1).wrapping_mul(n)
    } else {
        1
    }
}

fn iter_factorial(mut n: u32) -> u32 {
    let mut acc = n.max(1);
    while n > 1 {
        acc = acc.wrapping_mul(n - 1);
        n -= 1;
    }
    acc
}

// Define the benchmarks.
fn factorial_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("factorial");

    let new_mod =
        || JITModule::new(JITBuilder::new(cranelift_module::default_libcall_names()).unwrap());
    let mut module = new_mod();
    let mut ctx = module.make_context();

    let mut func: Function = cranelift_reader::parse_functions(CLIF_REC_FAC)
        .unwrap()
        .remove(0);

    let mut rec_factorial_clif: Option<extern "sysv64" fn(i32) -> i32> = None;
    group.bench_function("compile recursive factorial", |b| {
        b.iter_batched(
            || std::mem::replace(&mut module, new_mod()),
            |mut module| {
                let funcid = module.declare_anonymous_function(&func.signature).unwrap();

                let self_name = func.declare_imported_user_function(UserExternalName {
                    namespace: 0,
                    index: funcid.as_u32(),
                });
                let self_import = func.dfg.ext_funcs.get_mut(FuncRef::from_u32(0)).unwrap();
                self_import.name = ExternalName::User(self_name);

                module.clear_context(&mut ctx);
                std::mem::swap(&mut func, &mut ctx.func);
                module.define_function(funcid, &mut ctx).unwrap();
                std::mem::swap(&mut func, &mut ctx.func);

                module.finalize_definitions().unwrap();

                let ptr = module.get_finalized_function(funcid);

                rec_factorial_clif = Some(unsafe {
                    std::mem::transmute::<*const u8, extern "sysv64" fn(i32) -> i32>(ptr)
                });
            },
            BatchSize::PerIteration,
        )
    });

    group.bench_function("run recursive factorial", |b| {
        let fac = rec_factorial_clif.unwrap();
        b.iter(|| {
            assert_eq!(fac(30), 1_409_286_144);
        });
    });

    group.bench_function("rust recursive factorial", |b| {
        b.iter(|| {
            assert_eq!(rec_factorial(std::hint::black_box(30)), 1_409_286_144);
        });
    });

    let mut func: Function = cranelift_reader::parse_functions(CLIF_ITER_FAC)
        .unwrap()
        .remove(0);

    let mut iter_factorial_clif: Option<extern "sysv64" fn(i32) -> i32> = None;
    group.bench_function("compile iterative factorial", |b| {
        b.iter_batched(
            || std::mem::replace(&mut module, new_mod()),
            |mut module| {
                let funcid = module.declare_anonymous_function(&func.signature).unwrap();

                module.clear_context(&mut ctx);
                std::mem::swap(&mut func, &mut ctx.func);
                module.define_function(funcid, &mut ctx).unwrap();
                std::mem::swap(&mut func, &mut ctx.func);

                module.finalize_definitions().unwrap();

                let ptr = module.get_finalized_function(funcid);

                iter_factorial_clif = Some(unsafe {
                    std::mem::transmute::<*const u8, extern "sysv64" fn(i32) -> i32>(ptr)
                });
            },
            BatchSize::PerIteration,
        )
    });

    group.bench_function("run iterative factorial", |b| {
        let fac = iter_factorial_clif.unwrap();
        b.iter(|| {
            assert_eq!(fac(30), 1_409_286_144);
        });
    });

    group.bench_function("rust iterative factorial", |b| {
        b.iter(|| {
            assert_eq!(iter_factorial(std::hint::black_box(30)), 1_409_286_144);
        });
    });
}

criterion_group!(benches, factorial_benchmark);

criterion_main!(benches);
