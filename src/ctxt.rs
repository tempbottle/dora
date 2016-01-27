use libc::c_void;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use driver::cmd::Args;
use error::diag::Diagnostic;

use ast::*;
use ast::map::Map;
use interner::*;
use jit::fct::JitFct;
use jit::map::CodeMap;
use jit::stub::Stub;

use mem::Ptr;
use mir::Mir;

use sym::*;
use sym::Sym::*;
use ty::BuiltinType;

pub struct Context<'a, 'ast> where 'ast: 'a {
    pub args: &'a Args,
    pub interner: &'a Interner,
    pub map: &'a Map<'ast>,
    pub ast: &'ast Ast,
    pub diag: RefCell<Diagnostic>,
    pub sym: RefCell<SymTable>,

    // points from AST function definition node id to FctContextId
    pub fct_defs: RefCell<HashMap<NodeId, FctContextId>>,

    // stores all function definitions
    pub fcts: Vec<Arc<Mutex<FctContext<'ast>>>>,

    // stores all compiled functions
    pub code_map: RefCell<CodeMap>,
}

impl<'a, 'ast> Context<'a, 'ast> {
    pub fn new(args: &'a Args, interner: &'a Interner,
           map: &'a Map<'ast>, ast: &'ast Ast) -> Context<'a, 'ast> {
        Context {
            args: args,
            interner: interner,
            map: map,
            ast: ast,
            diag: RefCell::new(Diagnostic::new()),
            sym: RefCell::new(SymTable::new()),
            fct_defs: RefCell::new(HashMap::new()),
            fcts: Vec::new(),
            code_map: RefCell::new(CodeMap::new()),
        }
    }

    pub fn add_function(&mut self, mut fct: FctContext<'ast>) -> Result<FctContextId, Sym> {
        let name = fct.name;
        let fctid = FctContextId(self.fcts.len());

        fct.id = fctid;

        if let Some(ast) = fct.ast {
            assert!(self.fct_defs.borrow_mut().insert(ast.id, fctid).is_none());
        }

        self.fcts.push(Arc::new(Mutex::new(fct)));

        let mut sym = self.sym.borrow_mut();

        match sym.get(name) {
            Some(sym) => Err(sym),
            None => {
                assert!(sym.insert(name, SymFunction(fctid)).is_none());

                Ok(fctid)
            }
        }
    }

    pub fn fct_by_id<F, R>(&self, id: FctContextId, f: F) -> R where F: FnOnce(&FctContext<'ast>) -> R {
        let fct = self.fcts[id.0].clone();
        let fctxt = fct.lock().unwrap();

        f(&fctxt)
    }

    pub fn fct_by_id_mut<F, R>(&self, id: FctContextId, f: F) -> R where F: FnOnce(&mut FctContext<'ast>) -> R {
        let fct = self.fcts[id.0].clone();
        let mut fctxt = fct.lock().unwrap();

        f(&mut fctxt)
    }

    pub fn fct_by_node_id_mut<F, R>(&self, id: NodeId, f: F) -> R where F: FnOnce(&mut FctContext<'ast>) -> R {
        let map = self.fct_defs.borrow();
        let fct_id = *map.get(&id).unwrap();

        self.fct_by_id_mut(fct_id, f)
    }

    pub fn fct_by_node_id<F, R>(&self, id: NodeId, f: F) -> R where
                     F: FnOnce(&FctContext<'ast>) -> R {
        let map = self.fct_defs.borrow();
        let fct_id = *map.get(&id).unwrap();

        self.fct_by_id(fct_id, f)
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct FctContextId(pub usize);

#[derive(Debug)]
pub struct FctContext<'ast> {
    pub id: FctContextId,

    pub name: Name,

    pub params_types: Vec<BuiltinType>,

    pub return_type: BuiltinType,

    pub ast: Option<&'ast Function>,

    // maps function call to FctContextId
    pub calls: HashMap<NodeId, FctContextId>,

    // points to the definition of variable from its usage
    pub defs: HashMap<NodeId, VarContextId>,

    pub ir: Option<Mir>,

    // size of temporary variables on stack
    pub tempsize: i32,

    // size of local variables on stack
    pub localsize: i32,

    pub leaf: bool,

    pub vars: Vec<VarContext>,

    // true if function is always exited via return statement
    // false if function execution could reach the closing } of this function
    pub always_returns: bool,

    // ptr to machine code if already compiled
    pub code: FctCode,

    // compiler stub
    pub stub: Option<Stub>
}

impl<'ast> FctContext<'ast> {
    pub fn stacksize(&self) -> i32 {
        self.tempsize + self.localsize
    }

    pub fn var_by_node_id(&self, id: NodeId) -> &VarContext {
        let varid = *self.defs.get(&id).unwrap();

        &self.vars[varid.0]
    }

    pub fn var_by_node_id_mut(&mut self, id: NodeId) -> &mut VarContext {
        let varid = *self.defs.get(&id).unwrap();

        &mut self.vars[varid.0]
    }
}

#[derive(Debug)]
pub enum FctCode {
    Uncompiled, Builtin(Ptr), Fct(JitFct)
}

#[derive(Debug, PartialEq, Eq, Copy, Clone, Hash)]
pub struct VarContextId(pub usize);

#[derive(Debug)]
pub struct VarContext {
    pub id: VarContextId,

    pub name: Name,

    pub data_type: BuiltinType,

    pub node_id: NodeId,

    pub offset: i32,
}
