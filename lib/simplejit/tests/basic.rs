extern crate cranelift_codegen;
extern crate cranelift_entity;
extern crate cranelift_frontend;
extern crate cranelift_module;
extern crate cranelift_simplejit;

use cranelift_codegen::ir::*;
use cranelift_codegen::settings::*;
use cranelift_codegen::Context;
use cranelift_entity::EntityRef;
use cranelift_frontend::*;
use cranelift_module::*;
use cranelift_simplejit::*;

#[test]
fn error_on_incompatible_sig_in_declare_function() {
    let mut module: Module<SimpleJITBackend> = Module::new(SimpleJITBuilder::new());
    let mut sig = Signature {
        params: vec![AbiParam::new(types::I64)],
        returns: vec![],
        call_conv: CallConv::SystemV,
    };
    module
        .declare_function("abc", Linkage::Local, &sig)
        .unwrap();
    sig.params[0] = AbiParam::new(types::I32);
    module
        .declare_function("abc", Linkage::Local, &sig)
        .err()
        .unwrap(); // Make sure this is an error
}

fn define_simple_function(module: &mut Module<SimpleJITBackend>) -> FuncId {
    let sig = Signature {
        params: vec![],
        returns: vec![],
        call_conv: CallConv::SystemV,
    };

    let func_id = module
        .declare_function("abc", Linkage::Local, &sig)
        .unwrap();

    let mut ctx = Context::new();
    ctx.func = Function::with_name_signature(ExternalName::user(0, func_id.index() as u32), sig);
    let mut func_ctx = FunctionBuilderContext::new();
    {
        let mut bcx: FunctionBuilder = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let ebb = bcx.create_ebb();
        bcx.switch_to_block(ebb);
        bcx.ins().return_(&[]);
    }

    module.define_function(func_id, &mut ctx).unwrap();

    func_id
}

#[test]
fn double_finalize() {
    let mut module: Module<SimpleJITBackend> = Module::new(SimpleJITBuilder::new());

    define_simple_function(&mut module);
    module.finalize_definitions();

    // Calling `finalize_definitions` a second time without any new definitions
    // should have no effect.
    module.finalize_definitions();
}

#[test]
#[should_panic(expected = "Result::unwrap()` on an `Err` value: DuplicateDefinition(\"abc\")")]
fn panic_on_define_after_finalize() {
    let mut module: Module<SimpleJITBackend> = Module::new(SimpleJITBuilder::new());

    define_simple_function(&mut module);
    module.finalize_definitions();
    define_simple_function(&mut module);
}

#[test]
fn switch_error() {
    use cranelift_codegen::settings;

    let sig = Signature {
        params: vec![AbiParam::new(types::I32)],
        returns: vec![AbiParam::new(types::I32)],
        call_conv: CallConv::SystemV,
    };

    let mut func = Function::with_name_signature(ExternalName::user(0, 0), sig);

    let mut func_ctx = FunctionBuilderContext::new();
    {
        let mut bcx: FunctionBuilder = FunctionBuilder::new(&mut func, &mut func_ctx);
        let start = bcx.create_ebb();
        let bb0 = bcx.create_ebb();
        let bb1 = bcx.create_ebb();
        let bb2 = bcx.create_ebb();
        let bb3 = bcx.create_ebb();
        println!("{} {} {} {} {}", start, bb0, bb1, bb2, bb3);

        bcx.declare_var(Variable::new(0), types::I32);
        bcx.declare_var(Variable::new(1), types::I32);
        let in_val = bcx.append_ebb_param(start, types::I32);
        bcx.switch_to_block(start);
        bcx.def_var(Variable::new(0), in_val);
        bcx.ins().jump(bb0, &[]);

        bcx.switch_to_block(bb0);
        let discr = bcx.use_var(Variable::new(0));
        let mut switch = cranelift_frontend::Switch::new();
        for &(index, bb) in &[
            (9, bb1),
            (13, bb1),
            (10, bb1),
            (92, bb1),
            (39, bb1),
            (34, bb1),
        ] {
            switch.set_entry(index, bb);
        }
        switch.emit(&mut bcx, discr, bb2);

        bcx.switch_to_block(bb1);
        let v = bcx.use_var(Variable::new(0));
        bcx.def_var(Variable::new(1), v);
        bcx.ins().jump(bb3, &[]);

        bcx.switch_to_block(bb2);
        let v = bcx.use_var(Variable::new(0));
        bcx.def_var(Variable::new(1), v);
        bcx.ins().jump(bb3, &[]);

        bcx.switch_to_block(bb3);
        let r = bcx.use_var(Variable::new(1));
        bcx.ins().return_(&[r]);

        bcx.seal_all_blocks();
        bcx.finalize();
    }

    let flags = settings::Flags::new(settings::builder());
    match cranelift_codegen::verify_function(&func, &flags) {
        Ok(_) => {}
        Err(err) => {
            let pretty_error =
                cranelift_codegen::print_errors::pretty_verifier_error(&func, None, None, err);
            panic!("pretty_error:\n{}", pretty_error);
        }
    }
}
