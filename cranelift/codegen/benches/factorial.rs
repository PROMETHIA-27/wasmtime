//! Measure how quickly cranelift can compile and run 2 factorial functions, separately.
//! One function is defined recursively, one is defined iteratively. It also benches two equivalent
//! rust factorial functions, to compare.

use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::immediates::Imm64;
use cranelift_codegen::ir::{
    types, AbiParam, Function, InstructionData, Opcode, Signature, UserFuncName,
};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::Module;
use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

mod clifp;

// Define the benchmarks.
fn factorial_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("factorial");

    let new_mod =
        || JITModule::new(JITBuilder::new(cranelift_module::default_libcall_names()).unwrap());
    let mut module = new_mod();
    let mut ctx = module.make_context();

    println!("toks: {:?}", clifp::lex("(bruh 1 1.2)"));

    let mut rec_factorial_clif: Option<fn(i32) -> i32> = None;
    group.bench_function("compile recursive factorial", |b| {
        b.iter_batched(
            || std::mem::replace(&mut module, new_mod()),
            |mut module| {
                let mut sig = Signature::new(module.isa().default_call_conv());
                sig.params.push(AbiParam::new(types::I32));
                sig.returns.push(AbiParam::new(types::I32));

                let funcid = module.declare_anonymous_function(&sig).unwrap();

                let mut func = Function::with_name_signature(UserFuncName::default(), sig.clone());

                let import_func = module.declare_func_in_func(funcid, &mut func);

                let entry = func.dfg.make_block();
                func.layout.append_block(entry);

                func.dfg.append_block_param(entry, types::I32);
                let n = func.dfg.block_params(entry)[0];

                let nonzero_branch = func.dfg.make_block();
                func.layout.append_block(nonzero_branch);

                let zero_branch = func.dfg.make_block();
                func.layout.append_block(zero_branch);

                let zbranch_call = func.dfg.block_call(zero_branch, &[]);
                let nzbranch_call = func.dfg.block_call(nonzero_branch, &[]);

                // brif branches on an int, depending on whether it's 0 or not, so no math needed to calculate each branch
                let branch = func.dfg.make_inst(InstructionData::Brif {
                    opcode: Opcode::Brif,
                    arg: n,
                    blocks: [nzbranch_call, zbranch_call],
                });
                func.layout.append_inst(branch, entry);

                let one = func.dfg.make_inst(InstructionData::UnaryImm {
                    opcode: Opcode::Iconst,
                    imm: Imm64::new(1),
                });
                func.layout.append_inst(one, zero_branch);
                func.dfg.make_inst_results(one, types::I32);
                let one = func.dfg.inst_results_list(one);

                let ret = func.dfg.make_inst(InstructionData::MultiAry {
                    opcode: Opcode::Return,
                    args: one,
                });
                func.layout.append_inst(ret, zero_branch);

                let one = func.dfg.make_inst(InstructionData::UnaryImm {
                    opcode: Opcode::Iconst,
                    imm: Imm64::new(1),
                });
                func.layout.append_inst(one, nonzero_branch);
                func.dfg.make_inst_results(one, types::I32);
                let one = func.dfg.inst_results(one)[0];

                let nsub1 = func.dfg.make_inst(InstructionData::Binary {
                    opcode: Opcode::Isub,
                    args: [n, one],
                });
                func.layout.append_inst(nsub1, nonzero_branch);
                func.dfg.make_inst_results(nsub1, types::I32);
                let nsub1 = func.dfg.inst_results_list(nsub1);

                let reccall = func.dfg.make_inst(InstructionData::Call {
                    opcode: Opcode::Call,
                    args: nsub1,
                    func_ref: import_func,
                });
                func.layout.append_inst(reccall, nonzero_branch);
                func.dfg.make_inst_results(reccall, types::INVALID); // types inferred from func signature
                let nsub1fac = func.dfg.inst_results(reccall)[0];

                let nfac = func.dfg.make_inst(InstructionData::Binary {
                    opcode: Opcode::Imul,
                    args: [n, nsub1fac],
                });
                func.layout.append_inst(nfac, nonzero_branch);
                func.dfg.make_inst_results(nfac, types::I32);
                let nfac = func.dfg.inst_results_list(nfac);

                let ret = func.dfg.make_inst(InstructionData::MultiAry {
                    opcode: Opcode::Return,
                    args: nfac,
                });
                func.layout.append_inst(ret, nonzero_branch);

                module.clear_context(&mut ctx);
                ctx.func = func;
                module.define_function(funcid, &mut ctx).unwrap();

                module.finalize_definitions().unwrap();

                let ptr = module.get_finalized_function(funcid);

                rec_factorial_clif =
                    Some(unsafe { std::mem::transmute::<*const u8, fn(i32) -> i32>(ptr) });
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

    let mut iter_factorial_clif: Option<fn(i32) -> i32> = None;
    group.bench_function("compile iterative factorial", |b| {
        b.iter_batched(
            || std::mem::replace(&mut module, new_mod()),
            |mut module| {
                let mut sig = Signature::new(module.isa().default_call_conv());
                sig.params.push(AbiParam::new(types::I32));
                sig.returns.push(AbiParam::new(types::I32));

                let funcid = module.declare_anonymous_function(&sig).unwrap();

                let mut func = Function::with_name_signature(UserFuncName::default(), sig.clone());

                let entry = func.dfg.make_block();
                func.layout.append_block(entry);

                func.dfg.append_block_param(entry, types::I32);
                let n = func.dfg.block_params(entry)[0];

                let cmp = func.dfg.make_inst(InstructionData::IntCompareImm {
                    opcode: Opcode::IcmpImm,
                    arg: n,
                    cond: IntCC::UnsignedGreaterThan,
                    imm: Imm64::new(1),
                });

                module.clear_context(&mut ctx);
                ctx.func = func;
                module.define_function(funcid, &mut ctx).unwrap();

                module.finalize_definitions().unwrap();

                let ptr = module.get_finalized_function(funcid);

                rec_factorial_clif =
                    Some(unsafe { std::mem::transmute::<*const u8, fn(i32) -> i32>(ptr) });
            },
            BatchSize::PerIteration,
        )
    });

    group.bench_function("rust iterative factorial", |b| {
        b.iter(|| {
            assert_eq!(iter_factorial(std::hint::black_box(30)), 1_409_286_144);
        });
    });
}

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

criterion_group!(benches, factorial_benchmark);

criterion_main!(benches);
