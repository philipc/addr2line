use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::lazy::LazyResult;
use crate::{DebugFile, Error};

pub(crate) struct LazyNamespaces<R: gimli::Reader>(LazyResult<Namespaces<R>>);

impl<R: gimli::Reader> LazyNamespaces<R> {
    pub(crate) fn new() -> Self {
        Self(LazyResult::new())
    }

    pub(crate) fn borrow(
        &self,
        unit: &gimli::Unit<R>,
        sections: &gimli::Dwarf<R>,
    ) -> Result<&Namespaces<R>, Error> {
        self.0
            .borrow_with(|| Namespaces::parse(unit, sections))
            .as_ref()
            .map_err(Error::clone)
    }
}

pub(crate) struct Namespaces<R: gimli::Reader>(Box<[Namespace<R>]>);

impl<R: gimli::Reader> Namespaces<R> {
    pub(crate) fn new(namespaces: Vec<Namespace<R>>) -> Self {
        Namespaces(namespaces.into_boxed_slice())
    }

    fn parse(unit: &gimli::Unit<R>, sections: &gimli::Dwarf<R>) -> Result<Self, Error> {
        let mut namespaces = Vec::new();
        let mut entries = unit.entries_raw(None)?;
        while !entries.is_empty() {
            let depth = entries.next_depth();
            let dw_die_offset = entries.next_offset();
            if let Some(abbrev) = entries.read_abbreviation()? {
                if abbrev.tag() == gimli::DW_TAG_namespace {
                    for spec in abbrev.attributes() {
                        match entries.read_attribute(*spec) {
                            Ok(ref attr) => {
                                if attr.name() == gimli::DW_AT_name {
                                    let name = sections.attr_string(unit, attr.value())?;
                                    namespaces.push(Namespace {
                                        dw_die_offset,
                                        name,
                                        depth,
                                    });
                                }
                            }
                            Err(e) => return Err(e),
                        }
                    }
                } else {
                    entries.skip_attributes(abbrev.attributes())?;
                }
            }
        }

        Ok(Namespaces::new(namespaces))
    }

    pub(crate) fn find(
        &self,
        dw_die_offset: gimli::UnitOffset<R::Offset>,
    ) -> Result<NamespaceIter<R>, Error> {
        // TODO: Use binary search.
        let index = self
            .0
            .iter()
            .position(|namespace| namespace.dw_die_offset > dw_die_offset)
            .unwrap();
        Ok(NamespaceIter(&self.0[..index]))
    }
}

pub(crate) struct Namespace<R: gimli::Reader> {
    pub(crate) dw_die_offset: gimli::UnitOffset<R::Offset>,
    pub(crate) name: R,
    /// The depth of the namespace in the DIE tree.
    ///
    /// Used for backtracking up the tree to find the namespace of a function.
    pub(crate) depth: isize,
}

/// TODO
#[derive(Debug, Clone, Copy)]
pub struct NamespaceRef<T: gimli::ReaderOffset> {
    pub(crate) file: DebugFile,
    pub(crate) unit_index: usize,
    pub(crate) dw_die_offset: gimli::UnitOffset<T>,
}

impl<T: gimli::ReaderOffset> NamespaceRef<T> {
    pub(crate) fn new(
        file: DebugFile,
        unit_index: usize,
        dw_die_offset: gimli::UnitOffset<T>,
    ) -> Self {
        NamespaceRef {
            file,
            unit_index,
            dw_die_offset,
        }
    }
}

/// TODO
pub struct NamespaceIter<'a, R: gimli::Reader>(&'a [Namespace<R>]);

impl<'a, R: gimli::Reader> NamespaceIter<'a, R> {
    /// TODO
    pub fn next(&mut self) -> Option<Cow<'a, str>> {
        let namespace = self.0.last()?;
        while let Some(next) = self.0.last() {
            if next.depth < namespace.depth {
                break;
            }
            self.0 = &self.0[..self.0.len() - 1];
        }
        namespace.name.to_string_lossy().ok()
    }
}
