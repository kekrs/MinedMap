use std::{
	collections::{btree_map, BTreeMap},
	iter::{self, FusedIterator},
};

use anyhow::{bail, Context, Result};

use super::{de, section::*};
use crate::{resource::BlockTypes, types::*};

/// Chunk data structure wrapping a [de::Chunk] for convenient access to
/// block and biome data
#[derive(Debug)]
pub enum Chunk<'a> {
	/// Minecraft v1.18+ chunk with biome data moved into sections
	V1_18 {
		section_map: BTreeMap<SectionY, (SectionV1_13<'a>, BiomesV18<'a>, BlockLight<'a>)>,
	},
	/// Minecraft v1.13+ chunk
	///
	/// Block data is stored in an indexed format with variable bit width
	/// (depending on the total numer of distinct block types used in a
	/// section), and a palette mapping these indices to namespaced
	/// block IDs
	V1_13 {
		section_map: BTreeMap<SectionY, (SectionV1_13<'a>, BlockLight<'a>)>,
		biomes: &'a de::BiomesV0,
	},
	/// Original pre-1.13 chunk
	///
	/// The original chunk format with fixed 8-bit numeric block IDs
	V0 {
		section_map: BTreeMap<SectionY, (SectionV0<'a>, BlockLight<'a>)>,
		biomes: &'a de::BiomesV0,
	},
	/// Unpopulated chunk without any block data
	Empty,
}

/// Inner data structure of [SectionIter]
#[derive(Debug, Clone)]
enum SectionIterInner<'a> {
	/// Iterator over sections of [Chunk::V1_18]
	V1_18 {
		iter: btree_map::Iter<'a, SectionY, (SectionV1_13<'a>, BiomesV18<'a>, BlockLight<'a>)>,
	},
	/// Iterator over sections of [Chunk::V1_13]
	V1_13 {
		iter: btree_map::Iter<'a, SectionY, (SectionV1_13<'a>, BlockLight<'a>)>,
	},
	/// Iterator over sections of [Chunk::V0]
	V0 {
		iter: btree_map::Iter<'a, SectionY, (SectionV0<'a>, BlockLight<'a>)>,
	},
	/// Empty iterator over an unpopulated chunk ([Chunk::Empty])
	Empty,
}

/// Iterator over the sections of a [Chunk]
#[derive(Debug, Clone)]
pub struct SectionIter<'a> {
	inner: SectionIterInner<'a>,
}

impl<'a> Chunk<'a> {
	/// Creates a new [Chunk] from a deserialized [de::Chunk]
	pub fn new(data: &'a de::Chunk, block_types: &'a BlockTypes) -> Result<Self> {
		let data_version = data.data_version.unwrap_or_default();

		match &data.chunk {
			de::ChunkVariants::V1_18 { sections } => {
				Self::new_v1_18(data_version, sections, block_types)
			}
			de::ChunkVariants::V0 { level } => Self::new_v0(data_version, level, block_types),
		}
	}

	/// [Chunk::new] implementation for Minecraft v1.18+ chunks
	fn new_v1_18(
		data_version: u32,
		sections: &'a Vec<de::SectionV1_18>,
		block_types: &'a BlockTypes,
	) -> Result<Self> {
		let mut section_map = BTreeMap::new();

		for section in sections {
			section_map.insert(
				SectionY(section.y),
				(
					SectionV1_13::new(
						data_version,
						section.block_states.data.as_deref(),
						&section.block_states.palette,
						block_types,
					)
					.with_context(|| format!("Failed to load section at Y={}", section.y))?,
					BiomesV18::new(section.biomes.data.as_deref(), &section.biomes.palette)
						.with_context(|| {
							format!("Failed to load section biomes at Y={}", section.y)
						})?,
					BlockLight::new(section.block_light.as_deref()).with_context(|| {
						format!("Failed to load section block light at Y={}", section.y)
					})?,
				),
			);
		}

		Ok(Chunk::V1_18 { section_map })
	}

	/// [Chunk::new] implementation for all pre-1.18 chunk variants
	fn new_v0(
		data_version: u32,
		level: &'a de::LevelV0,
		block_types: &'a BlockTypes,
	) -> Result<Self> {
		let mut section_map_v1_13 = BTreeMap::new();
		let mut section_map_v0 = BTreeMap::new();

		for section in &level.sections {
			let block_light =
				BlockLight::new(section.block_light.as_deref()).with_context(|| {
					format!("Failed to load section block light at Y={}", section.y)
				})?;
			match &section.section {
				de::SectionV0Variants::V1_13 {
					block_states,
					palette,
				} => {
					section_map_v1_13.insert(
						SectionY(section.y.into()),
						(
							SectionV1_13::new(
								data_version,
								Some(block_states),
								palette,
								block_types,
							)
							.with_context(|| {
								format!("Failed to load section at Y={}", section.y)
							})?,
							block_light,
						),
					);
				}
				de::SectionV0Variants::V0 { blocks, data } => {
					section_map_v0.insert(
						SectionY(section.y.into()),
						(
							SectionV0::new(blocks, data, block_types).with_context(|| {
								format!("Failed to load section at Y={}", section.y)
							})?,
							block_light,
						),
					);
				}
				de::SectionV0Variants::Empty {} => {}
			}
		}

		// TODO Check biomes length
		let biomes = level.biomes.as_ref().context("Invalid biome data");

		Ok(
			match (section_map_v1_13.is_empty(), section_map_v0.is_empty()) {
				(true, true) => Chunk::Empty,
				(false, true) => Chunk::V1_13 {
					section_map: section_map_v1_13,
					biomes: biomes?,
				},
				(true, false) => Chunk::V0 {
					section_map: section_map_v0,
					biomes: biomes?,
				},
				(false, false) => {
					bail!("Mixed section versions");
				}
			},
		)
	}

	/// Returns an interator over the chunk's sections and their Y coordinates
	pub fn sections(&self) -> SectionIter {
		use SectionIterInner::*;
		SectionIter {
			inner: match self {
				Chunk::V1_18 { section_map } => V1_18 {
					iter: section_map.iter(),
				},
				Chunk::V1_13 { section_map, .. } => V1_13 {
					iter: section_map.iter(),
				},
				Chunk::V0 { section_map, .. } => V0 {
					iter: section_map.iter(),
				},
				Chunk::Empty => Empty,
			},
		}
	}
}

#[derive(Debug, Clone, Copy)]
pub struct SectionIterItem<'a> {
	pub y: SectionY,
	pub section: &'a dyn Section,
}

trait SectionIterTrait<'a>:
	Iterator<Item = SectionIterItem<'a>> + DoubleEndedIterator + ExactSizeIterator + FusedIterator
{
}

impl<'a, T> SectionIterTrait<'a> for T where
	T: Iterator<Item = SectionIterItem<'a>>
		+ DoubleEndedIterator
		+ ExactSizeIterator
		+ FusedIterator
{
}

impl<'a> SectionIter<'a> {
	fn with_iter<F, T>(&mut self, f: F) -> T
	where
		F: FnOnce(&mut dyn SectionIterTrait<'a>) -> T,
	{
		match &mut self.inner {
			SectionIterInner::V1_18 { iter } => {
				f(&mut iter.map(|(&y, (section, _, _))| SectionIterItem { y, section }))
			}
			SectionIterInner::V1_13 { iter } => {
				f(&mut iter.map(|(&y, (section, _))| SectionIterItem { y, section }))
			}
			SectionIterInner::V0 { iter } => {
				f(&mut iter.map(|(&y, (section, _))| SectionIterItem { y, section }))
			}
			SectionIterInner::Empty => f(&mut iter::empty()),
		}
	}
}

impl<'a> Iterator for SectionIter<'a> {
	type Item = SectionIterItem<'a>;

	fn next(&mut self) -> Option<Self::Item> {
		self.with_iter(|iter| iter.next())
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		match &self.inner {
			SectionIterInner::V1_18 { iter } => iter.size_hint(),
			SectionIterInner::V1_13 { iter } => iter.size_hint(),
			SectionIterInner::V0 { iter } => iter.size_hint(),
			SectionIterInner::Empty => (0, Some(0)),
		}
	}

	fn last(mut self) -> Option<Self::Item> {
		self.with_iter(|iter| iter.last())
	}
}

impl<'a> DoubleEndedIterator for SectionIter<'a> {
	fn next_back(&mut self) -> Option<Self::Item> {
		self.with_iter(|iter| iter.next_back())
	}
}

impl<'a> ExactSizeIterator for SectionIter<'a> {
	fn len(&self) -> usize {
		match &self.inner {
			SectionIterInner::V1_18 { iter } => iter.len(),
			SectionIterInner::V1_13 { iter } => iter.len(),
			SectionIterInner::V0 { iter } => iter.len(),
			SectionIterInner::Empty => 0,
		}
	}
}

impl<'a> FusedIterator for SectionIter<'a> {}
