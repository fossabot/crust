use crate::lexer::TokType;
use crate::parser::{NodeType, ParseNode, StmtType};
use std::collections::{HashMap, HashSet};

// generate a std::String contains the assembly language code
static mut LABEL_COUNTER: i64 = -1;
fn gen_labels(prefix: String) -> String {
    unsafe {
        LABEL_COUNTER = LABEL_COUNTER + 1;
        return format!(".L{}{}", prefix, LABEL_COUNTER);
    }
}

static mut FLAG_FOR_MAIN_HAS_RET: bool = true;
fn fn_main_has_ret() {
    unsafe {
        FLAG_FOR_MAIN_HAS_RET = true;
    }
}

fn gen_fn_prologue(fn_name: String) -> String {
    let p = "        ";
    format!(
        "{}.global {}\n\
         {}.type {}, @function\n\
         {}:\n\
         {}:\n\
         {}.cfi_startproc\n\
         {}pushq	%rbp\n\
         {}.cfi_def_cfa_offset 16\n\
         {}.cfi_offset 6, -16\n\
         {}movq	%rsp, %rbp\n\
         {}.cfi_def_cfa_register 6\n\
         ",
        p,
        fn_name,
        p,
        fn_name,
        fn_name,
        gen_labels("FB".to_string()),
        p,
        p,
        p,
        p,
        p,
        p
    )
}

fn gen_fn_epilogue() -> String {
    let p = "        ";
    format!(
        "{}movq %rbp, %rsp\n\
         {}popq	%rbp\n\
         {}.cfi_def_cfa 7, 8\n",
        p, p, p
    )
}
pub fn gen_prog(tree: &ParseNode) -> String {
    let p = "        ".to_string();

    // iter every function node
    let mut prog_body = String::new();
    let index_map: HashMap<String, isize> = HashMap::new();
    let idx: isize = 0;
    for it in tree.child.iter() {
        match &it.entry {
            NodeType::Fn(fn_name, _) => {
                let fn_prologue = gen_fn_prologue(fn_name.to_string());
                let fn_epilogue = gen_fn_epilogue();
                let fn_body = &gen_block(it, &index_map, idx);
                let tmp = unsafe {
                    if FLAG_FOR_MAIN_HAS_RET == false {
                        format!(
                            "{}movq $0, %rax\n\
                             {}\
                             {}ret\n",
                            p,
                            gen_fn_epilogue(),
                            p
                        )
                    } else {
                        "".to_string()
                    }
                };
                let fn_tot = format!(
                    "{}\
                     {}\
                     {}\
                     {}\
                     {}.cfi_endproc\n\
                     {}:\n\
                     {}.size   {}, .-{}\n",
                    fn_prologue,
                    fn_body,
                    tmp,
                    fn_epilogue,
                    p,
                    gen_labels("FE".to_string()),
                    p,
                    fn_name,
                    fn_name
                );
                prog_body.push_str(&fn_tot);
            }
            _ => panic!("`{:?}` type should not be here", it.entry),
        }
    }
    match &tree.entry {
        NodeType::Prog(prog_name) => format!(
            "{}.file \"{}\"\n\
             {}\
             {}.ident	\"crust: 0.1 (By Haoran Wang)\"\n\
             {}.section	.note.GNU-stack,\"\",@progbits\n",
            p, prog_name, prog_body, p, p
        ),
        _ => panic!("Something went wrong in gen_prog"),
    }
}

pub fn gen_declare(
    tree: &ParseNode,
    index_map: &HashMap<String, isize>,
    scope: &HashSet<String>,
    idx: isize,
) -> (HashMap<String, isize>, HashSet<String>, isize, String) {
    let p = "        ";
    let mut index_map = index_map.clone();
    let mut scope = scope.clone();
    let mut idx = idx;
    match &tree.entry {
        NodeType::Declare(var_name) => {
            if scope.contains(var_name) {
                panic!(
                    "Error: redeclaration of variable `{}` in the same scope",
                    var_name
                );
            } else {
                let tmp_str = format!("{}", var_name);
                scope.insert(tmp_str);
                // try to clear the previous index
                let tmp_str = format!("{}", var_name);
                index_map.insert(tmp_str, idx - 8);
                idx -= 8;
            }

            // judge whether it's initialized
            let mut e1 = String::new();

            if tree.child.is_empty() {
                // just declare, we initialized it with 0
                e1 = format!("        movq $0, %rax\n");
            } else {
                e1 = gen_stmt(
                    tree.child
                        .get(0)
                        .expect("Statement::Declare Node has no child"),
                    &index_map,
                    idx,
                )
            }
            let s = format!(
                "{}\
                 {}pushq %rax\n",
                e1, p
            );
            (index_map, scope, idx, s)
        }
        _ => panic!("Type `{:?}` should not occur here", tree.entry),
    }
}

/// gen_block() - into a new block, will have empty scope
pub fn gen_block(tree: &ParseNode, index_map: &HashMap<String, isize>, idx: isize) -> String {
    let p = "        ".to_string(); // 8 white spaces

    // iter every block
    let mut stmts = String::new();
    let mut index_map = index_map.clone();
    let mut idx: isize = idx;
    let mut scope: HashSet<String> = HashSet::new();

    for it in &tree.child {
        // iter through every block-item
        match &it.entry {
            NodeType::Declare(_var_name) => {
                let (index_map_new, scope_new, idx_new, s) =
                    gen_declare(it, &index_map, &scope, idx);
                index_map = index_map_new.clone();
                idx = idx_new;
                scope = scope_new.clone();
                stmts.push_str(&s);
            }
            NodeType::Stmt(StmtType::Compound) => {
                stmts.push_str(&gen_block(it, &index_map, idx));
            }
            _ => {
                let s = gen_stmt(it, &index_map, idx);
                stmts.push_str(&s);
            }
        }
    }
    let b_deallocate = 8 * scope.len(); // deallocate stack
    format!(
        "{}\
         {}addq ${}, %rsp\n",
        stmts, p, b_deallocate
    )
}

pub fn gen_stmt(tree: &ParseNode, index_map: &HashMap<String, isize>, idx: isize) -> String {
    let p = "        ".to_string(); // 8 white spaces
    match &tree.entry {
        NodeType::ConditionalExp => {
            if tree.child.len() == 1 {
                // just one <logical-or-exp>
                gen_stmt(
                    tree.child
                        .get(0)
                        .expect("Conditional Expression has no child"),
                    index_map,
                    idx,
                )
            } else if tree.child.len() == 3 {
                // <logical-or-exp> "?" <exp> ":" <conditional-exp>
                let e1_as = gen_stmt(
                    tree.child.get(0).expect("Conditional expression no e1"),
                    index_map,
                    idx,
                );
                let e2_as = gen_stmt(
                    tree.child.get(1).expect("conditional expression no e2"),
                    index_map,
                    idx,
                );
                let e3_as = gen_stmt(
                    tree.child.get(2).expect("conditional expression no e3"),
                    index_map,
                    idx,
                );

                let label_e3 = gen_labels(format!("E3"));
                let label_end = gen_labels(format!("ENDCOND"));
                format!(
                    "{}\
                     {}cmpq $0, %rax\n\
                     {}je {}\n\
                     {}\
                     {}jmp {}\n\
                     {}:\n\
                     {}\
                     {}:\n",
                    e1_as, p, p, label_e3, e2_as, p, label_end, label_e3, e3_as, label_end,
                )
            } else {
                panic!("Error: something wrong in conditional expression")
            }
        }
        NodeType::Stmt(stmt) => match stmt {
            StmtType::Return => format!(
                "{}\
                 {}\
                 {}ret\n",
                gen_stmt(
                    tree.child.get(0).expect("Statement node no child"),
                    index_map,
                    idx
                ),
                gen_fn_epilogue(),
                p
            ),
            StmtType::Conditional(_) => {
                let e1_as = gen_stmt(
                    tree.child.get(0).expect("Conditional node no e1"),
                    index_map,
                    idx,
                );
                let s1_as = gen_stmt(
                    tree.child.get(1).expect("conditional node no s1"),
                    index_map,
                    idx,
                );
                let s2_as: String = if tree.child.len() == 2 {
                    "".to_string()
                } else {
                    gen_stmt(
                        tree.child.get(2).expect("conditional node no s2"),
                        index_map,
                        idx,
                    )
                };
                let label_s2 = gen_labels(format!("S2"));
                let label_end = gen_labels(format!("ENDIF"));
                format!(
                    "{}\
                     {}cmpq $0, %rax\n\
                     {}je {}\n\
                     {}\
                     {}jmp {}\n\
                     {}:\n\
                     {}\
                     {}:\n",
                    e1_as, p, p, label_s2, s1_as, p, label_end, label_s2, s2_as, label_end,
                )
            }
            StmtType::Exp => gen_stmt(
                tree.child.get(0).expect("Statement Node no child"),
                index_map,
                idx,
            ),
            StmtType::Compound => gen_block(tree, index_map, idx),
            _ => panic!("Compound should not occur here"),
        },
        NodeType::AssignNode(var_name) => {
            match index_map.get(var_name) {
                Some(t) => {
                    // declared before, that's ok
                    let e1 = gen_stmt(
                        tree.child
                            .get(0)
                            .expect("Statement::Declare Node has no child"),
                        index_map,
                        idx,
                    );
                    let get_result = index_map.get(var_name);
                    let mut va_offset: isize = -8;
                    match get_result {
                        Some(t) => {
                            va_offset = *t;
                        }
                        None => panic!("Something went wrong in gen::gen_stmt()"),
                    }
                    format!(
                        "{}\
                         {}movq %rax, {}(%rbp)\n",
                        e1, p, va_offset
                    )
                }
                None => {
                    // Not declared before, that's not ok
                    panic!("Error: Use un-declared variable `{}`", var_name)
                }
            }
        }
        NodeType::UnExp(Op) => match Op {
            TokType::Minus => format!(
                "{}\
                 {}neg %rax\n",
                gen_stmt(
                    tree.child.get(0).expect("UnExp<-> no child"),
                    index_map,
                    idx
                ),
                p
            ),
            TokType::Tilde => format!(
                "{}\
                 {}not %rax\n",
                gen_stmt(
                    tree.child.get(0).expect("UnExp<~> no child"),
                    index_map,
                    idx
                ),
                p
            ),
            TokType::Exclamation => format!(
                "{}\
                 {}cmp  $0, %rax\n\
                 {}movq $0, %rax\n\
                 {}sete %al\n",
                gen_stmt(
                    tree.child.get(0).expect("UnExp<!> node no child"),
                    index_map,
                    idx
                ),
                p,
                p,
                p
            ),
            TokType::Lt => format!("Error: `<` not implemented"),
            TokType::Gt => format!("Error: `>` not implemented"),
            _ => panic!(format!(
                "Unary Operator `{:?}` not implemented in gen::gen_unexp()\n",
                Op
            )),
        },
        NodeType::BinExp(Op) => {
            match Op {
                TokType::Plus => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}addq %rcx, %rax\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp has no lhs"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp has no rhs"),
                        index_map,
                        idx
                    ),
                    p,
                    p
                ),
                TokType::Minus => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}subq %rcx, %rax\n", // subl src, dst : dst - src -> dst
                    //   let %rax = dst = e1, %rcx = src = e2
                    gen_stmt(
                        tree.child.get(1).expect("BinExp has no rhs"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(0).expect("BinExp has no lhs"),
                        index_map,
                        idx
                    ),
                    p,
                    p
                ),
                TokType::Multi => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}imul %rcx, %rax\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp has no lhs"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp has no rhs"),
                        index_map,
                        idx
                    ),
                    p,
                    p
                ),
                TokType::Splash => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}xorq %rdx, %rdx\n\
                     {}idivq %rcx\n",
                    // let eax = e1, edx = 0, ecx = e2
                    gen_stmt(
                        tree.child.get(1).expect("BinExp has no rhs"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(0).expect("BinExp has no lhs"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p
                ),
                TokType::Equal => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}sete %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                TokType::NotEqual => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}setne %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                TokType::LessEqual => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}setle %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                TokType::GreaterEqual => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}setge %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                TokType::Or => {
                    let clause2_label = gen_labels(format!("CLAUSE"));
                    let end_label = gen_labels(format!("END"));
                    format!(
                        "{}\
                         {}cmpq $0, %rax\n\
                         {}je {}\n\
                         {}movq $1, %rax\n\
                         {}jmp {}\n\
                         {}:\n\
                         {}\
                         {}cmpq $0, %rax\n\
                         {}movq $0, %rax\n\
                         {}setne %al\n\
                         {}: # end of clause here\n",
                        gen_stmt(
                            tree.child.get(0).expect("BinExp<||> node no child"),
                            index_map,
                            idx
                        ),
                        p,
                        p,
                        clause2_label,
                        p,
                        p,
                        end_label,
                        clause2_label,
                        gen_stmt(
                            tree.child.get(1).expect("BinExp<||> node no child"),
                            index_map,
                            idx
                        ),
                        p,
                        p,
                        p,
                        end_label
                    )
                }
                TokType::And => {
                    let clause2_label = gen_labels(format!("clause"));
                    let end_label = gen_labels(format!("end"));
                    format!(
                        "{}\
                         {}cmpq $0, %rax\n\
                         {}jne {}\n\
                         {}jmp {}\n\
                         {}:\n\
                         {}\
                         {}cmpq $0, %rax\n\
                         {}movq $0, %rax\n\
                         {}setne %al\n\
                         {}: # end of clause here\n",
                        gen_stmt(
                            tree.child.get(0).expect("BinExp<||> node no child"),
                            index_map,
                            idx
                        ),
                        p,
                        p,
                        clause2_label,
                        p,
                        end_label,
                        clause2_label,
                        gen_stmt(
                            tree.child.get(1).expect("BinExp<||> node no child"),
                            index_map,
                            idx
                        ),
                        p,
                        p,
                        p,
                        end_label
                    )
                }
                TokType::Lt => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}setl %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                TokType::Gt => format!(
                    "{}\
                     {}pushq %rax\n\
                     {}\
                     {}popq %rcx\n\
                     {}cmpq %rax, %rcx # set ZF on if %rax == %rcx, set it off otherwise\n\
                     {}movq $0, %rax   # zero out EAX, does not change flag\n\
                     {}setg %al\n",
                    gen_stmt(
                        tree.child.get(0).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    gen_stmt(
                        tree.child.get(1).expect("BinExp<==> node no child"),
                        index_map,
                        idx
                    ),
                    p,
                    p,
                    p,
                    p
                ),
                _ => panic!(format!(
                    "Error: Binary Operator `{:?}` not implemented in gen::gen_binexp()\n",
                    Op
                )),
            }
        }
        NodeType::Const(n) => format!("{}movq ${}, %rax\n", p, n),
        NodeType::Var(var_name) => {
            let var_offset = index_map.get(var_name);
            match var_offset {
                Some(t) => {
                    let var_offset = t;
                    format!("{}movq {}(%rbp), %rax\n", p, var_offset)
                }
                None => panic!(format!("Use of undeclared variable `{}`", var_name)),
            }
        }
        NodeType::EqualityExp
        | NodeType::RelationalExp
        | NodeType::Term
        | NodeType::Exp
        | NodeType::Factor
        | NodeType::AdditiveExp
        | NodeType::LogicalOrExp
        | NodeType::Block
        | NodeType::LogicalAndExp => gen_stmt(
            tree.child
                .get(0)
                .expect(&format!("{:?} node no child", &tree.entry)),
            index_map,
            idx,
        ),
        _ => panic!(format!(
            "Node `{:?}` not implemented in gen::gen_stmt()\n",
            &tree.entry
        )),
    }
}
