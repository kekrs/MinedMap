use std::iter;

use anyhow::Result;

/// A generic array of 3x3 elements
///
/// A RegionGroup is used to store information about a 3x3 neighbourhood of
/// regions.
///
/// The center element is always populated, while the 8 adjacent elements may be None.
#[derive(Debug, Clone, Copy)]
pub struct RegionGroup<T> {
	center: T,
	neighs: [Option<T>; 9],
}

impl<T> RegionGroup<T> {
	pub fn new<F>(f: F) -> Result<Self>
	where
		F: Fn(i8, i8) -> Result<T>,
	{
		RegionGroup {
			center: (0, 0),
			neighs: [
				Some((-1, -1)),
				Some((-1, 0)),
				Some((-1, 1)),
				Some((0, -1)),
				None,
				Some((0, 1)),
				Some((1, -1)),
				Some((1, 0)),
				Some((1, 1)),
			],
		}
		.try_map(|(x, z)| f(x, z))
	}

	pub fn center(&self) -> &T {
		&self.center
	}

	pub fn get(&self, x: i8, z: i8) -> Option<&T> {
		if (x, z) == (0, 0) {
			return Some(&self.center);
		}
		if !(-1..=1).contains(&x) || !(-1..=1).contains(&z) {
			return None;
		}
		self.neighs.get((3 * x + z + 4) as usize)?.as_ref()
	}

	pub fn map<U, F>(self, mut f: F) -> RegionGroup<U>
	where
		F: FnMut(T) -> U,
	{
		RegionGroup {
			center: f(self.center),
			neighs: self.neighs.map(|entry| entry.map(&mut f)),
		}
	}

	pub fn try_map<U, F>(self, mut f: F) -> Result<RegionGroup<U>>
	where
		F: FnMut(T) -> Result<U>,
	{
		let RegionGroup { center, neighs } = self;
		let center = f(center)?;
		let neighs = neighs.map(|entry| entry.and_then(|value| f(value).ok()));
		Ok(RegionGroup { center, neighs })
	}

	pub fn iter(&self) -> impl Iterator<Item = &T> {
		iter::once(&self.center).chain(self.neighs.iter().filter_map(Option::as_ref))
	}
}
