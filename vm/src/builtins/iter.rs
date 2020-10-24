/*
 * iterator types
 */

use crossbeam_utils::atomic::AtomicCell;

use super::pytype::PyTypeRef;
use crate::pyobject::{
    PyCallable, PyClassImpl, PyContext, PyObjectRef, PyRef, PyResult, PyValue, TypeProtocol,
};
use crate::vm::VirtualMachine;

#[pyclass(module = false, name = "iter")]
#[derive(Debug)]
pub struct PySequenceIterator {
    pub position: AtomicCell<isize>,
    pub obj: PyObjectRef,
    pub reversed: bool,
}

impl PyValue for PySequenceIterator {
    fn class(vm: &VirtualMachine) -> &PyTypeRef {
        &vm.ctx.types.iter_type
    }
}

#[pyimpl]
impl PySequenceIterator {
    pub fn new_forward(obj: PyObjectRef) -> Self {
        Self {
            position: AtomicCell::new(0),
            obj,
            reversed: false,
        }
    }

    pub fn new_reversed(obj: PyObjectRef, len: isize) -> Self {
        Self {
            position: AtomicCell::new(len - 1),
            obj,
            reversed: true,
        }
    }

    #[pymethod(name = "__next__")]
    fn next(&self, vm: &VirtualMachine) -> PyResult {
        let step: isize = if self.reversed { -1 } else { 1 };
        let pos = self.position.fetch_add(step);
        if pos >= 0 {
            match vm.call_method(&self.obj, "__getitem__", (pos,)) {
                Err(ref e) if e.isinstance(&vm.ctx.exceptions.index_error) => {
                    Err(vm.new_stop_iteration())
                }
                // also catches stop_iteration => stop_iteration
                ret => ret,
            }
        } else {
            Err(vm.new_stop_iteration())
        }
    }

    #[pymethod(name = "__iter__")]
    fn iter(zelf: PyRef<Self>) -> PyRef<Self> {
        zelf
    }

    #[pymethod(name = "__length_hint__")]
    fn length_hint(&self, vm: &VirtualMachine) -> PyResult<isize> {
        let pos = self.position.load();
        let hint = if self.reversed {
            pos + 1
        } else {
            let len = vm._len(&self.obj).unwrap_or_else(|| {
                Err(vm.new_type_error("sequence has no __len__ method".to_owned()))
            })?;
            len as isize - pos
        };
        Ok(hint)
    }
}

#[pyclass(module = false, name = "callable_iterator")]
#[derive(Debug)]
pub struct PyCallableIterator {
    callable: PyCallable,
    sentinel: PyObjectRef,
    done: AtomicCell<bool>,
}

impl PyValue for PyCallableIterator {
    fn class(vm: &VirtualMachine) -> &PyTypeRef {
        &vm.ctx.types.callable_iterator
    }
}

#[pyimpl]
impl PyCallableIterator {
    pub fn new(callable: PyCallable, sentinel: PyObjectRef) -> Self {
        Self {
            callable,
            sentinel,
            done: AtomicCell::new(false),
        }
    }

    #[pymethod(magic)]
    fn next(&self, vm: &VirtualMachine) -> PyResult {
        if self.done.load() {
            return Err(vm.new_stop_iteration());
        }

        let ret = self.callable.invoke((), vm)?;

        if vm.bool_eq(&ret, &self.sentinel)? {
            self.done.store(true);
            Err(vm.new_stop_iteration())
        } else {
            Ok(ret)
        }
    }

    #[pymethod(magic)]
    fn iter(zelf: PyRef<Self>) -> PyRef<Self> {
        zelf
    }
}

pub fn init(context: &PyContext) {
    PySequenceIterator::extend_class(context, &context.types.iter_type);
    PyCallableIterator::extend_class(context, &context.types.callable_iterator);
}
