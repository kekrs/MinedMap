use crate::types::*;

pub trait ShiftMask: Sized {
	type MaskedOutput;

	/// Apply a right shift to a value, and return both the result and the
	/// bytes that were shifted out
	fn shift_mask(self, shift: u8) -> (Self, Self::MaskedOutput);
}

impl ShiftMask for u32 {
	type MaskedOutput = u32;

	fn shift_mask(self, shift: u8) -> (u32, u32) {
		let mask = (1 << shift) - 1;
		(self >> shift, self & mask)
	}
}

impl ShiftMask for i32 {
	type MaskedOutput = u32;

	#[inline]
	fn shift_mask(self, shift: u8) -> (i32, u32) {
		let mask = (1 << shift) - 1;
		(self >> shift, (self as u32) & mask)
	}
}

/// Offsets a chunk and block coordinate pair by a number of blocks
///
/// As the new coordinate may end up in a different region, a region offset
/// is returned together with the new chunk and block coordinates.
#[inline]
pub fn coord_offset<const AXIS: u8>(
	chunk: ChunkCoord<AXIS>,
	block: BlockCoord<AXIS>,
	offset: i32,
) -> (i8, ChunkCoord<AXIS>, BlockCoord<AXIS>) {
	let coord = ((chunk.0 as i32) << BLOCK_BITS | block.0 as i32) + offset;
	let (region_chunk, block) = coord.shift_mask(BLOCK_BITS);
	let (region, chunk) = region_chunk.shift_mask(CHUNK_BITS);
	(
		region
			.try_into()
			.expect("the region coordinate should be in the valid range"),
		ChunkCoord::new(chunk),
		BlockCoord::new(block),
	)
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_coord_offset() {
		const CHUNKS: i32 = CHUNKS_PER_REGION as i32;
		const BLOCKS: i32 = BLOCKS_PER_CHUNK as i32;

		for chunk in ChunkX::iter() {
			for block in BlockX::iter() {
				assert_eq!(coord_offset(chunk, block, 0), (0, chunk, block));
				assert_eq!(
					coord_offset(chunk, block, -(CHUNKS * BLOCKS)),
					(-1, chunk, block)
				);
				assert_eq!(
					coord_offset(chunk, block, CHUNKS * BLOCKS),
					(1, chunk, block)
				);

				for offset in -(CHUNKS * BLOCKS)..(CHUNKS * BLOCKS) {
					let (region2, chunk2, block2) = coord_offset(chunk, block, offset);
					assert!((-1..=1).contains(&region2));
					let coord = chunk.0 as i32 * BLOCKS + block.0 as i32 + offset;
					let coord2 =
						((region2 as i32 * CHUNKS) + chunk2.0 as i32) * BLOCKS + block2.0 as i32;
					assert_eq!(coord2, coord);
				}
			}
		}
	}
}
