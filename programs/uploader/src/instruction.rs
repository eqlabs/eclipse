use {
    nom::{
        branch::alt,
        bytes::complete::{tag, take},
        number::complete::u32,
        Compare, CompareResult, Finish, IResult, InputLength,
    },
    num_enum::TryFromPrimitive,
    solana_program::program_error::ProgramError,
};

#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Copy, Clone, TryFromPrimitive)]
enum InstructionType {
    CreateBucket,
    PutIntoBucket,
}

impl InputLength for InstructionType {
    fn input_len(&self) -> usize {
        1
    }
}

impl Compare<InstructionType> for &[u8] {
    fn compare(&self, t: InstructionType) -> CompareResult {
        if self.is_empty() {
            return CompareResult::Incomplete;
        }

        if self[0] == t as u8 {
            CompareResult::Ok
        } else {
            CompareResult::Error
        }
    }

    fn compare_no_case(&self, t: InstructionType) -> CompareResult {
        self.compare(t)
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ProgramInstruction<'a> {
    /// Create a bucket for data.
    ///
    /// # Account references
    ///   0. `[SIGNER, WRITE]` Account used to derive and control the new data bucket.
    ///   1. `[SIGNER, WRITE]` Account that will fund the new data bucket.
    ///   2. `[WRITE]` Uninitialized data bucket account
    ///   3. `[]` System program for CPI.
    CreateBucket {
        /// Seed data (the first bytes) to put into the bucket.
        data: &'a [u8],

        /// Total size of data bucket.
        size: u32,

        /// Data buckets are always initialized at program-derived
        /// addresses using the funding address, recent blockhash, and
        /// the user-passed `bump_seed`.
        bump_seed: u8,
    },

    /// Put data into bucket.
    ///
    /// # Account references
    ///   0. `[SIGNER, WRITE]` Account used to control the new data bucket.
    ///   1. `[SIGNER, WRITE]` Account that will fund the extension of data bucket.
    ///   2. `[WRITE]` Data bucket account.
    ///   3. `[]` System program for CPI.
    PutIntoBucket {
        /// Data to put into the bucket.
        data: &'a [u8],

        /// Offset
        offset: u32,
    },
}

fn instruction_type<'a>(input: &'a [u8]) -> IResult<&'a [u8], InstructionType> {
    let (rest, kind): (&[u8], &[u8]) = alt((
        tag(InstructionType::CreateBucket),
        tag(InstructionType::PutIntoBucket),
    ))(input)?;

    Ok((
        rest,
        InstructionType::try_from(kind[0])
            .expect("unknown instruction type slipped past parsing choice"),
    ))
}

fn be_u32(input: &[u8]) -> IResult<&[u8], u32> {
    Ok(u32(nom::number::Endianness::Big)(input)?)
}

fn create_bucket(input: &[u8]) -> IResult<&[u8], ProgramInstruction> {
    let (input, size) = be_u32(input)?;
    let (data, bump_seed) = take(1usize)(input)?;

    Ok((
        data,
        ProgramInstruction::CreateBucket {
            data,
            size,
            bump_seed: bump_seed[0],
        },
    ))
}

fn put_into_bucket(input: &[u8]) -> IResult<&[u8], ProgramInstruction> {
    let (data, offset) = be_u32(input)?;

    Ok((data, ProgramInstruction::PutIntoBucket { data, offset }))
}

pub fn parse_program_instruction<'a>(
    instruction_data: &'a [u8],
) -> Result<ProgramInstruction, ProgramError> {
    let (rest, it) = instruction_type(instruction_data).finish().unwrap();
    match it {
        InstructionType::CreateBucket => {
            let (_, bucket) = create_bucket(rest).unwrap();
            Ok(bucket)
        }
        InstructionType::PutIntoBucket => {
            let (_, bucket) = put_into_bucket(rest).unwrap();
            Ok(bucket)
        }
    }
}

impl ProgramInstruction<'_> {
    pub fn serialize(&self) -> Vec<u8> {
        match self {
            Self::CreateBucket {
                data,
                size,
                bump_seed,
            } => [
                &[InstructionType::CreateBucket as u8],
                size.to_be_bytes().as_slice(),
                &[*bump_seed],
                data,
            ]
            .concat()
            .to_vec(),
            Self::PutIntoBucket { data, offset } => [
                &[InstructionType::PutIntoBucket as u8],
                offset.to_be_bytes().as_slice(),
                data,
            ]
            .concat()
            .to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn test_serialize_and_deserialize_create() {
        let orig = ProgramInstruction::CreateBucket {
            data: &[1, 2, 3, 4, 5, 6],
            size: 80,
            bump_seed: 42,
        };

        let bs = orig.serialize();
        let new = parse_program_instruction(bs.as_ref()).unwrap();

        assert_eq!(orig, new);
    }

    #[test]
    fn test_serialize_and_deserialize_putinto() {
        let orig = ProgramInstruction::PutIntoBucket {
            data: &[7, 8, 9, 10, 11, 12],
            offset: 6,
        };

        let bs = orig.serialize();
        let new = parse_program_instruction(bs.as_ref()).unwrap();

        assert_eq!(orig, new);
    }
}
