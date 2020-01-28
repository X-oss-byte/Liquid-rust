use std::fmt;
use std::slice;

use itertools;
use kstring::KStringCow;
use liquid_error::{Error, Result};

use crate::ScalarCow;
use crate::ValueView;

/// Path to a value in an `Object`.
///
/// There is guaranteed always at least one element.
#[derive(Clone, Debug, PartialEq)]
pub struct Path<'s>(Vec<ScalarCow<'s>>);

impl<'s> Path<'s> {
    /// Create a `Value` reference.
    pub fn with_index<I: Into<ScalarCow<'s>>>(value: I) -> Self {
        let indexes = vec![value.into()];
        Path(indexes)
    }

    /// Append an index.
    pub fn push<I: Into<ScalarCow<'s>>>(&mut self, value: I) {
        self.0.push(value.into());
    }

    /// Reserves capacity for at least `additional` more elements to be inserted
    /// in the given `Path`. The `Path` may reserve more space to avoid
    /// frequent reallocations. After calling `reserve`, capacity will be
    /// greater than or equal to `self.len() + additional`. Does nothing if
    /// capacity is already sufficient.
    pub fn reserve(&mut self, additional: usize) {
        self.0.reserve(additional);
    }

    /// Access the `Value` reference.
    pub fn iter(&self) -> PathIter<'_, '_> {
        PathIter(self.0.iter())
    }

    /// Extracts a slice containing the entire vector.
    #[inline]
    pub fn as_slice(&self) -> &[ScalarCow<'s>] {
        self.0.as_slice()
    }
}

impl<'s> Extend<ScalarCow<'s>> for Path<'s> {
    fn extend<T: IntoIterator<Item = ScalarCow<'s>>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

impl<'s> ::std::ops::Deref for Path<'s> {
    type Target = [ScalarCow<'s>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'s> ::std::borrow::Borrow<[ScalarCow<'s>]> for Path<'s> {
    #[inline]
    fn borrow(&self) -> &[ScalarCow<'s>] {
        self
    }
}

impl<'s> AsRef<[ScalarCow<'s>]> for Path<'s> {
    #[inline]
    fn as_ref(&self) -> &[ScalarCow<'s>] {
        self
    }
}

impl<'s> fmt::Display for Path<'s> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = itertools::join(self.iter().map(ValueView::render), ".");
        write!(f, "{}", data)
    }
}

/// Iterate over indexes in a `Value`'s `Path`.
#[derive(Debug)]
pub struct PathIter<'i, 's>(slice::Iter<'i, ScalarCow<'s>>);

impl<'i, 's: 'i> Iterator for PathIter<'i, 's> {
    type Item = &'i ScalarCow<'s>;

    #[inline]
    fn next(&mut self) -> Option<&'i ScalarCow<'s>> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }
}

impl<'i, 's: 'i> ExactSizeIterator for PathIter<'i, 's> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

/// Path to a value in an `Object`.
pub type PathRef<'p, 's> = &'p [ScalarCow<'s>];

/// Find a `ValueView` nested in an `ObjectView`
pub fn try_find<'o>(value: &'o dyn ValueView, path: PathRef<'_, '_>) -> Option<&'o dyn ValueView> {
    let indexes = path.iter();
    indexes.fold(Some(value), |value, index| {
        let value = value?;
        if let Some(arr) = value.as_array() {
            if let Some(index) = index.to_integer() {
                arr.get(index)
            } else {
                match &*index.to_kstr() {
                    "first" => arr.first(),
                    "last" => arr.last(),
                    _ => None,
                }
            }
        } else if let Some(obj) = value.as_object() {
            obj.get(index.to_kstr().as_str())
        } else {
            None
        }
    })
}

/// Find a `ValueView` nested in an `ObjectView`
pub fn find<'o>(value: &'o dyn ValueView, path: PathRef<'_, '_>) -> Result<&'o dyn ValueView> {
    if let Some(res) = try_find(value, path) {
        Ok(res)
    } else {
        for cur_idx in 1..path.len() {
            let subpath_end = path.len() - cur_idx;
            let subpath = &path[0..subpath_end];
            if let Some(parent) = try_find(value, subpath) {
                let subpath = itertools::join(subpath.iter().map(ValueView::render), ".");
                let requested = &path[subpath_end];
                let available = if let Some(arr) = parent.as_array() {
                    let mut available = vec![
                        KStringCow::from_static("first"),
                        KStringCow::from_static("last"),
                    ];
                    if 0 < arr.size() {
                        available
                            .insert(0, KStringCow::from_string(format!("0..{}", arr.size() - 1)));
                    }
                    available
                } else if let Some(obj) = parent.as_object() {
                    let available: Vec<_> = obj.keys().collect();
                    available
                } else {
                    Vec::new()
                };
                let available = itertools::join(available.iter(), ", ");
                return Error::with_msg("Unknown index")
                    .context("variable", subpath)
                    .context("requested index", format!("{}", requested.render()))
                    .context("available indexes", available)
                    .into_err();
            }
        }

        panic!(
            "Should have already errored for `{}` with path {:?}",
            value.source(),
            path
        );
    }
}
