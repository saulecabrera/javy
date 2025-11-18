use crate::readers::BinaryReader;
use crate::{AtomIndex, ClosureVarIndex, ConstantPoolIndex, LocalIndex};
use anyhow::{Result, bail};

/// A QuickJS operator code.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
pub enum Opcode {
    /// A marker, never emitted.
    Invalid = 0,
    /// Push an `i32` value.
    PushI32 {
        value: i32,
    },
    /// Push a constant value.
    PushConst {
        /// The index of the constant in the constant pool.
        index: ConstantPoolIndex,
    },
    /// Push a function closure value.
    FClosure {
        /// The index of the closure in the constant pool.
        index: ConstantPoolIndex,
    },
    /// Push an atom constant.
    PushAtomValue {
        /// The immediate value of the atom.
        atom: AtomIndex,
    },
    /// Push a private symbol from an atom immediate.
    PrivateSymbol {
        /// The immediate value of the atom.
        atom: AtomIndex,
    },
    /// Push undefined value.
    Undefined,
    /// Push a null value.
    Null,
    /// Push the current object.
    PushThis,
    /// Push a false constant.
    PushFalse,
    /// Puhs a true constant.
    PushTrue,
    /// Push a new object.
    Object,
    /// Push a special object.
    SpecialObject {
        /// The special object argument.
        argument: i32,
    },
    // TODO: Verify this.
    /// Rest arguments.
    Rest {
        /// The first argument.
        first: u16,
    },
    /// Drop the top value.
    Drop,
    /// Drop the second top value.
    Nip,
    /// Drop the third top value.
    Nip1,
    /// Duplicate the top value, pushing the new value at the stack top.
    Dup,
    /// Similar to [Opcode::Dup] but puts the new value in the second top most
    /// position.
    Dup1,
    /// Duplicate the top two values, pushing the new values at the stack top.
    Dup2,
    /// Duplicate the top three values pushing the values at the stack top.
    Dup3,

    // TODO: Skipping comments for now.
    Insert2,
    Insert3,
    Insert4,
    Perm3,
    Perm4,
    Perm5,
    Swap,
    Swap2,
    Rot3L,
    Rot3R,
    Rot4L,
    Rot5L,

    CallConstructor {
        argc: u16,
    },
    Call {
        argc: u16,
    },
    TailCall {
        argc: u16,
    },
    CallMethod {
        argc: u16,
    },
    TailCallMethod {
        argc: u16,
    },

    ArrayFrom {
        argc: u16,
    },
    Apply {
        magic: u16,
    },
    Return,
    ReturnUndef,
    CheckCtorReturn,
    CheckCtor,
    InitCtor,
    CheckBrand,
    AddBrand,
    ReturnAsync,
    Throw,
    ThrowError {
        ty: u8,
        atom: AtomIndex,
    },
    Eval {
        scope: u16,
        argc: u16,
    },
    ApplyEval {
        scope: u16,
    },
    Regexp,

    GetSuper,
    Import,

    CheckVar {
        atom: AtomIndex,
    },
    GetVarUndef {
        atom: AtomIndex,
    },
    GetVar {
        atom: AtomIndex,
    },
    PutVar {
        atom: AtomIndex,
    },
    PutVarInit {
        atom: AtomIndex,
    },
    PutVarStrict {
        atom: AtomIndex,
    },
    GetRefValue,
    PutRefValue,
    DefineVar {
        flags: u8,
        atom: AtomIndex,
    },
    CheckDefineVar {
        flags: u8,
        atom: AtomIndex,
    },
    DefineFunc {
        flags: u8,
        atom: AtomIndex,
    },
    GetField {
        atom: AtomIndex,
    },
    GetField2 {
        atom: AtomIndex,
    },
    PutField {
        atom: AtomIndex,
    },
    GetPrivateField,
    PutPrivateField,
    DefinePrivateField,
    GetArrayEl,
    GetArrayEl2,
    PutArrayEl,
    GetSuperValue,
    PutSuperValue,
    DefineField {
        atom: AtomIndex,
    },
    SetName {
        atom: AtomIndex,
    },
    SetNameComputed,
    SetProto,
    SetHomeObject,
    DefineArrayEl,
    Append,
    CopyDataProperties {
        mask: u8,
    },
    DefineMethod {
        atom: AtomIndex,
        flags: u8,
    },
    DefineMethodComputed {
        flags: u8,
    },
    DefineClass {
        flags: u8,
        atom: AtomIndex,
    },
    DefineClassComputed {
        flags: u8,
        atom: AtomIndex,
    },
    GetLoc {
        // index to local variable list (after arg list)
        index: LocalIndex,
    },
    PutLoc {
        index: LocalIndex,
    },
    SetLoc {
        index: LocalIndex,
    },
    GetArg {
        // index to arg list
        index: LocalIndex,
    },
    PutArg {
        index: LocalIndex,
    },
    SetArg {
        index: LocalIndex,
    },
    GetVarRef {
        // index to the closures list
        index: ClosureVarIndex,
    },
    PutVarRef {
        index: ClosureVarIndex,
    },
    SetVarRef {
        index: ClosureVarIndex,
    },
    SetLocUninit {
        index: LocalIndex,
    },
    GetLocCheck {
        index: LocalIndex,
    },
    PutLocCheck {
        index: LocalIndex,
    },
    PutLocCheckInit {
        index: LocalIndex,
    },
    GetVarRefCheck {
        index: ClosureVarIndex,
    },
    PutVarRefCheck {
        index: ClosureVarIndex,
    },
    PutVarRefCheckInit {
        index: ClosureVarIndex,
    },
    CloseLoc {
        // TODO: figure out what this is
        index: u16,
    },
    IfFalse {
        offset: u32,
    },
    IfTrue {
        offset: u32,
    },
    GoTo {
        offset: u32,
    },
    Catch {
        diff: u32,
    },
    GoSub {
        diff: u32,
    },
    Ret,
    NipCatch,
    ToObject,
    ToPropKey,
    ToPropKey2,
    WithGetVar {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    WithPutVar {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    WithDeleteVar {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    WithMakeRef {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    WithGetRef {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    WithGetRefUndef {
        atom: AtomIndex,
        diff: u32,
        is_with: u8,
    },
    MakeLocRef {
        atom: AtomIndex,
        idx: u16,
    },
    MakeArgRef {
        atom: AtomIndex,
        idx: u16,
    },
    MakeVarRefRef {
        atom: AtomIndex,
        idx: u16,
    },
    MakeVarRef {
        atom: AtomIndex,
    },
    ForInStart,
    ForOfStart,
    ForAwaitOfStart,
    ForInNext,
    ForOfNext {
        offset: u32,
    },
    IteratorCheckObject,
    IteratorGetValueDone,
    IteratorClose,
    IteratorNext,
    IteratorCall {
        flags: u8,
    },
    InitialYield,
    Yield,
    YieldStar,
    AsyncYieldStar,
    Await,
    Neg,
    Plus,
    Dec,
    Inc,
    PostDec,
    PostInc,
    DecLoc {
        index: LocalIndex,
    },
    IncLoc {
        index: LocalIndex,
    },
    AddLoc {
        index: LocalIndex,
    },
    Not,
    LNot,
    TypeOf,
    Delete,
    DeleteVar {
        atom: AtomIndex,
    },
    Mul,
    Div,
    Mod,
    Add,
    Sub,
    Shl,
    Sar,
    Shr,
    And,
    Xor,
    Or,
    Pow,
    Lt,
    Lte,
    Gt,
    Gte,
    InstanceOf,
    In,
    Eq,
    Neq,
    StrictEq,
    StrictNeq,
    UndefOrNull,
    PrivateIn,
    PushBigintI32 {
        value: i32,
    },
    // Short opcodes.
    Nop,
    PushMinus1,
    Push0,
    Push1,
    Push2,
    Push3,
    Push4,
    Push5,
    Push6,
    Push7,
    PushI8 {
        val: i8,
    },
    PushI16 {
        val: i16,
    },
    PushConst8 {
        index: u8,
    },
    FClosure8 {
        index: ConstantPoolIndex,
    },
    PushEmptyString,
    GetLoc8 {
        index: LocalIndex,
    },
    PutLoc8 {
        index: LocalIndex,
    },
    SetLoc8 {
        index: LocalIndex,
    },
    GetLoc0Loc1,
    GetLoc0,
    GetLoc1,
    GetLoc2,
    GetLoc3,
    PutLoc0,
    PutLoc1,
    PutLoc2,
    PutLoc3,
    SetLoc0,
    SetLoc1,
    SetLoc2,
    SetLoc3,
    GetArg0,
    GetArg1,
    GetArg2,
    GetArg3,
    PutArg0,
    PutArg1,
    PutArg2,
    PutArg3,
    SetArg0,
    SetArg1,
    SetArg2,
    SetArg3,
    GetVarRef0,
    GetVarRef1,
    GetVarRef2,
    GetVarRef3,
    PutVarRef0,
    PutVarRef1,
    PutVarRef2,
    PutVarRef3,
    SetVarRef0,
    SetVarRef1,
    SetVarRef2,
    SetVarRef3,
    GetLength,
    IfFalse8 {
        offset: u32,
    },
    IfTrue8 {
        offset: u32,
    },
    GoTo8 {
        offset: u32,
    },
    GoTo16 {
        offset: u32,
    },
    Call0,
    Call1,
    Call2,
    Call3,
    IsUndefined,
    IsNull,
    TypeOfIsUndefined,
    TypeOfIsFunction,
}

impl Opcode {
    /// reads an opcode, with immediates from a buffer, and returns the parsed opcode object.
    pub fn from_reader(reader: &mut BinaryReader<'_>) -> Result<(u32, Opcode)> {
        use Opcode::*;
        // The start of the operator.
        let pc = reader.offset as u32;
        // Reader is advanced 1 byte.
        let byte = reader.read_u8()?;
        let op = match byte {
            0 => Invalid,
            1 => PushI32 {
                value: i32::try_from(reader.read_u32()?)?,
            },
            2 => PushConst {
                index: ConstantPoolIndex::from_u32(reader.read_u32()?),
            },
            3 => FClosure {
                index: ConstantPoolIndex::from_u32(reader.read_u32()?),
            },
            4 => PushAtomValue {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            5 => PrivateSymbol {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            6 => Undefined,
            7 => Null,
            8 => PushThis,
            9 => PushFalse,
            10 => PushTrue,
            11 => Object,
            12 => SpecialObject {
                argument: reader.read_u8()? as i32,
            },
            13 => Rest {
                first: reader.read_u16()?,
            },
            14 => Drop,
            15 => Nip,
            16 => Nip1,
            17 => Dup,
            18 => Dup1,
            19 => Dup2,
            20 => Dup3,
            21 => Insert2,
            22 => Insert3,
            23 => Insert4,
            24 => Perm3,
            25 => Perm4,
            26 => Perm5,
            27 => Swap,
            28 => Swap2,
            29 => Rot3L,
            30 => Rot3R,
            31 => Rot4L,
            32 => Rot5L,
            33 => CallConstructor {
                argc: reader.read_u16()?,
            },
            34 => Call {
                argc: reader.read_u16()?,
            },
            35 => TailCall {
                argc: reader.read_u16()?,
            },
            36 => CallMethod {
                argc: reader.read_u16()?,
            },
            37 => TailCallMethod {
                argc: reader.read_u16()?,
            },
            38 => ArrayFrom {
                argc: reader.read_u16()?,
            },
            39 => Apply {
                magic: reader.read_u16()?,
            },
            40 => Return,
            41 => ReturnUndef,
            42 => CheckCtorReturn,
            43 => CheckCtor,
            44 => InitCtor,
            45 => CheckBrand,
            46 => AddBrand,
            47 => ReturnAsync,
            48 => Throw,
            49 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let ty = reader.read_u8()?;
                ThrowError { atom, ty }
            }
            50 => {
                let argc = reader.read_u16()?;
                let scope = reader.read_u16()? - 1;
                Eval { scope, argc }
            }
            51 => ApplyEval {
                scope: reader.read_u16()? - 1,
            },
            52 => Regexp,
            53 => GetSuper,
            54 => Import,
            55 => CheckVar {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            56 => GetVarUndef {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            57 => GetVar {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            58 => PutVar {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            59 => PutVarInit {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            60 => PutVarStrict {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            61 => GetRefValue,
            62 => PutRefValue,
            63 | 64 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let flags = reader.read_u8()?;
                if byte == 63 {
                    DefineVar { flags, atom }
                } else {
                    CheckDefineVar { flags, atom }
                }
            }
            65 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let flags = reader.read_u8()?;
                DefineFunc { flags, atom }
            }
            66 => GetField {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            67 => GetField2 {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            68 => PutField {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            69 => GetPrivateField,
            70 => PutPrivateField,
            71 => DefinePrivateField,
            72 => GetArrayEl,
            73 => GetArrayEl2,
            74 => PutArrayEl,
            75 => GetSuperValue,
            76 => PutSuperValue,
            77 => DefineField {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            78 => SetName {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            79 => SetNameComputed,
            80 => SetProto,
            81 => SetHomeObject,
            82 => DefineArrayEl,
            83 => Append,
            84 => CopyDataProperties {
                mask: reader.read_u8()?,
            },
            85 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let flags = reader.read_u8()?;
                DefineMethod { atom, flags }
            }
            86 => DefineMethodComputed {
                flags: reader.read_u8()?,
            },
            87 | 88 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let flags = reader.read_u8()?;
                if byte == 87 {
                    DefineClass { atom, flags }
                } else {
                    DefineClassComputed { atom, flags }
                }
            }
            89 => GetLoc {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            90 => PutLoc {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            91 => SetLoc {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            92 => GetArg {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            93 => PutArg {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            94 => SetArg {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            95 => GetVarRef {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            96 => PutVarRef {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            97 => SetVarRef {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            98 => SetLocUninit {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            99 => GetLocCheck {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            100 => PutLocCheck {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            101 => PutLocCheckInit {
                index: LocalIndex::from_u32(reader.read_u16()? as u32),
            },
            102 => GetVarRefCheck {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            103 => PutVarRefCheck {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            104 => PutVarRefCheckInit {
                index: ClosureVarIndex::from_u32(reader.read_u16()? as u32),
            },
            105 => CloseLoc {
                index: reader.read_u16()?,
            },
            106 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u32()?;
                IfFalse {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            107 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u32()?;
                IfTrue {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            108 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u32()?;
                GoTo {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            109 => Catch {
                diff: reader.read_u32()?,
            },
            110 => GoSub {
                diff: reader.read_u32()?,
            },
            111 => Ret,
            112 => NipCatch,
            113 => ToObject,
            114 => ToPropKey,
            115 => ToPropKey2,
            116..=121 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let diff = reader.read_u32()?;
                let is_with = reader.read_u8()?;
                match byte {
                    116 => WithGetVar {
                        atom,
                        diff,
                        is_with,
                    },
                    117 => WithPutVar {
                        atom,
                        diff,
                        is_with,
                    },
                    118 => WithDeleteVar {
                        atom,
                        diff,
                        is_with,
                    },
                    119 => WithMakeRef {
                        atom,
                        diff,
                        is_with,
                    },
                    120 => WithGetRef {
                        atom,
                        diff,
                        is_with,
                    },
                    121 => WithGetRefUndef {
                        atom,
                        diff,
                        is_with,
                    },
                    _ => unreachable!(),
                }
            }
            122 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let idx = reader.read_u16()?;
                MakeLocRef { atom, idx }
            }
            123 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let idx = reader.read_u16()?;
                MakeArgRef { atom, idx }
            }
            124 => {
                let atom = AtomIndex::from_u32(reader.read_u32()?);
                let idx = reader.read_u16()?;
                MakeVarRefRef { atom, idx }
            }
            125 => MakeVarRef {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            126 => ForInStart,
            127 => ForOfStart,
            128 => ForAwaitOfStart,
            129 => ForInNext,
            130 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u8()? as u32;
                ForOfNext {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            131 => IteratorCheckObject,
            132 => IteratorGetValueDone,
            133 => IteratorClose,
            134 => IteratorNext,
            135 => IteratorCall {
                flags: reader.read_u8()?,
            },
            136 => InitialYield,
            137 => Yield,
            138 => YieldStar,
            139 => AsyncYieldStar,
            140 => Await,
            141 => Neg,
            142 => Plus,
            143 => Dec,
            144 => Inc,
            145 => PostDec,
            146 => PostInc,
            147 => DecLoc {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            148 => IncLoc {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            149 => AddLoc {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            150 => Not,
            151 => LNot,
            152 => TypeOf,
            153 => Delete,
            154 => DeleteVar {
                atom: AtomIndex::from_u32(reader.read_u32()?),
            },
            155 => Mul,
            156 => Div,
            157 => Mod,
            158 => Add,
            159 => Sub,
            160 => Shl,
            161 => Sar,
            162 => Shr,
            163 => And,
            164 => Xor,
            165 => Or,
            166 => Pow,
            167 => Lt,
            168 => Lte,
            169 => Gt,
            170 => Gte,
            171 => InstanceOf,
            172 => In,
            173 => Eq,
            174 => Neq,
            175 => StrictEq,
            176 => StrictNeq,
            177 => UndefOrNull,
            178 => PrivateIn,
            179 => PushBigintI32 {
                value: i32::try_from(reader.read_u32()?)?,
            },
            180 => Nop,
            181 => PushMinus1,
            182 => Push0,
            183 => Push1,
            184 => Push2,
            185 => Push3,
            186 => Push4,
            187 => Push5,
            188 => Push6,
            189 => Push7,
            190 => PushI8 {
                val: reader.read_u8()? as i8,
            },
            191 => PushI16 {
                val: reader.read_u16()? as i16,
            },
            192 => PushConst8 {
                index: reader.read_u8()?,
            },
            193 => FClosure8 {
                index: ConstantPoolIndex::from_u32(reader.read_u8()? as u32),
            },
            194 => PushEmptyString,
            195 => GetLoc8 {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            196 => PutLoc8 {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            197 => SetLoc8 {
                index: LocalIndex::from_u32(reader.read_u8()? as u32),
            },
            198 => GetLoc0Loc1,
            199 => GetLoc0,
            200 => GetLoc1,
            201 => GetLoc2,
            202 => GetLoc3,
            203 => PutLoc0,
            204 => PutLoc1,
            205 => PutLoc2,
            206 => PutLoc3,
            207 => SetLoc0,
            208 => SetLoc1,
            209 => SetLoc2,
            210 => SetLoc3,
            211 => GetArg0,
            212 => GetArg1,
            213 => GetArg2,
            214 => GetArg3,
            215 => PutArg0,
            216 => PutArg1,
            217 => PutArg2,
            218 => PutArg3,
            219 => SetArg0,
            220 => SetArg1,
            221 => SetArg2,
            222 => SetArg3,
            223 => GetVarRef0,
            224 => GetVarRef1,
            225 => GetVarRef2,
            226 => GetVarRef3,
            227 => PutVarRef0,
            228 => PutVarRef1,
            229 => PutVarRef2,
            230 => PutVarRef3,
            231 => SetVarRef0,
            232 => SetVarRef1,
            233 => SetVarRef2,
            234 => SetVarRef3,
            235 => GetLength,
            236 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u8()? as u32;
                IfFalse8 {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            237 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u8()? as u32;
                IfTrue8 {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            238 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u8()? as u32;
                GoTo8 {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            239 => {
                let pc = reader.offset as u32;
                let offset = reader.read_u16()? as u32;
                GoTo16 {
                    offset: pc.checked_add(offset).unwrap(),
                }
            }
            240 => Call0,
            241 => Call1,
            242 => Call2,
            243 => Call3,
            244 => IsUndefined,
            245 => IsNull,
            246 => TypeOfIsUndefined,
            247 => TypeOfIsFunction,
            x => bail!("Unsupported opcode {x}"),
        };
        Ok((pc, op))
    }

    /// returns the canonical name of the opcode from byte value, without immediates.
    pub fn name_from_byte(byte: u8) -> String {
        match byte {
            0 => "Invalid",
            1 => "PushI32",
            2 => "PushConst",
            3 => "FClosure",
            4 => "PushAtomValue",
            5 => "PrivateSymbol",
            6 => "Undefined",
            7 => "Null",
            8 => "PushThis",
            9 => "PushFalse",
            10 => "PushTrue",
            11 => "Object",
            12 => "SpecialObject",
            13 => "Rest",
            14 => "Drop",
            15 => "Nip",
            16 => "Nip1",
            17 => "Dup",
            18 => "Dup1",
            19 => "Dup2",
            20 => "Dup3",
            21 => "Insert2",
            22 => "Insert3",
            23 => "Insert4",
            24 => "Perm3",
            25 => "Perm4",
            26 => "Perm5",
            27 => "Swap",
            28 => "Swap2",
            29 => "Rot3L",
            30 => "Rot3R",
            31 => "Rot4L",
            32 => "Rot5L",
            33 => "CallConstructor",
            34 => "Call",
            35 => "TailCall",
            36 => "CallMethod",
            37 => "TailCallMethod",
            38 => "ArrayFrom",
            39 => "Apply",
            40 => "Return",
            41 => "ReturnUndef",
            42 => "CheckCtorReturn",
            43 => "CheckCtor",
            44 => "InitCtor",
            45 => "CheckBrand",
            46 => "AddBrand",
            47 => "ReturnAsync",
            48 => "Throw",
            49 => "ThrowError",
            50 => "Eval",
            51 => "ApplyEval",
            52 => "Regexp",
            53 => "GetSuper",
            54 => "Import",
            55 => "CheckVar",
            56 => "GetVarUndef",
            57 => "GetVar",
            58 => "PutVar",
            59 => "PutVarInit",
            60 => "PutVarStrict",
            61 => "GetRefValue",
            62 => "PutRefValue",
            63 => "DefineVar",
            64 => "CheckDefineVar",
            65 => "DefineFunc",
            66 => "GetField",
            67 => "GetField2",
            68 => "PutField",
            69 => "GetPrivateField",
            70 => "PutPrivateField",
            71 => "DefinePrivateField",
            72 => "GetArrayEl",
            73 => "GetArrayEl2",
            74 => "PutArrayEl",
            75 => "GetSuperValue",
            76 => "PutSuperValue",
            77 => "DefineField",
            78 => "SetName",
            79 => "SetNameComputed",
            80 => "SetProto",
            81 => "SetHomeObject",
            82 => "DefineArrayEl",
            83 => "Append",
            84 => "CopyDataProperties",
            85 => "DefineMethod",
            86 => "DefineMethodComputed",
            87 => "DefineClass",
            88 => "DefineClassComputed",
            89 => "GetLoc",
            90 => "PutLoc",
            91 => "SetLoc",
            92 => "GetArg",
            93 => "PutArg",
            94 => "SetArg",
            95 => "GetVarRef",
            96 => "PutVarRef",
            97 => "SetVarRef",
            98 => "SetLocUninit",
            99 => "GetLocCheck",
            100 => "PutLocCheck",
            101 => "PutLocCheckInit",
            102 => "GetVarRefCheck",
            103 => "PutVarRefCheck",
            104 => "PutVarRefCheckInit",
            105 => "CloseLoc",
            106 => "IfFalse",
            107 => "IfTrue",
            108 => "GoTo",
            109 => "Catch",
            110 => "GoSub",
            111 => "Ret",
            112 => "NipCatch",
            113 => "ToObject",
            114 => "ToPropKey",
            115 => "ToPropKey2",
            116 => "WithGetVar",
            117 => "WithPutVar",
            118 => "WithDeleteVar",
            119 => "WithMakeRef",
            120 => "WithGetRef",
            121 => "WithGetRefUndef",
            122 => "MakeLocRef",
            123 => "MakeArgRef",
            124 => "MakeVarRefRef",
            125 => "MakeVarRef",
            126 => "ForInStart",
            127 => "ForOfStart",
            128 => "ForAwaitOfStart",
            129 => "ForInNext",
            130 => "ForOfNext",
            131 => "IteratorCheckObject",
            132 => "IteratorGetValueDone",
            133 => "IteratorClose",
            134 => "IteratorNext",
            135 => "IteratorCall",
            136 => "InitialYield",
            137 => "Yield",
            138 => "YieldStar",
            139 => "AsyncYieldStar",
            140 => "Await",
            141 => "Neg",
            142 => "Plus",
            143 => "Dec",
            144 => "Inc",
            145 => "PostDec",
            146 => "PostInc",
            147 => "DecLoc",
            148 => "IncLoc",
            149 => "AddLoc",
            150 => "Not",
            151 => "LNot",
            152 => "TypeOf",
            153 => "Delete",
            154 => "DeleteVar",
            155 => "Mul",
            156 => "Div",
            157 => "Mod",
            158 => "Add",
            159 => "Sub",
            160 => "Shl",
            161 => "Sar",
            162 => "Shr",
            163 => "And",
            164 => "Xor",
            165 => "Or",
            166 => "Pow",
            167 => "Lt",
            168 => "Lte",
            169 => "Gt",
            170 => "Gte",
            171 => "InstanceOf",
            172 => "In",
            173 => "Eq",
            174 => "Neq",
            175 => "StrictEq",
            176 => "StrictNeq",
            177 => "UndefOrNull",
            178 => "PrivateIn",
            179 => "PushBigintI32",
            180 => "Nop",
            181 => "PushMinus1",
            182 => "Push0",
            183 => "Push1",
            184 => "Push2",
            185 => "Push3",
            186 => "Push4",
            187 => "Push5",
            188 => "Push6",
            189 => "Push7",
            190 => "PushI8",
            191 => "PushI16",
            192 => "PushConst8",
            193 => "FClosure8",
            194 => "PushEmptyString",
            195 => "GetLoc8",
            196 => "PutLoc8",
            197 => "SetLoc8",
            198 => "GetLoc0Loc1",
            199 => "GetLoc0",
            200 => "GetLoc1",
            201 => "GetLoc2",
            202 => "GetLoc3",
            203 => "PutLoc0",
            204 => "PutLoc1",
            205 => "PutLoc2",
            206 => "PutLoc3",
            207 => "SetLoc0",
            208 => "SetLoc1",
            209 => "SetLoc2",
            210 => "SetLoc3",
            211 => "GetArg0",
            212 => "GetArg1",
            213 => "GetArg2",
            214 => "GetArg3",
            215 => "PutArg0",
            216 => "PutArg1",
            217 => "PutArg2",
            218 => "PutArg3",
            219 => "SetArg0",
            220 => "SetArg1",
            221 => "SetArg2",
            222 => "SetArg3",
            223 => "GetVarRef0",
            224 => "GetVarRef1",
            225 => "GetVarRef2",
            226 => "GetVarRef3",
            227 => "PutVarRef0",
            228 => "PutVarRef1",
            229 => "PutVarRef2",
            230 => "PutVarRef3",
            231 => "SetVarRef0",
            232 => "SetVarRef1",
            233 => "SetVarRef2",
            234 => "SetVarRef3",
            235 => "GetLength",
            236 => "IfFalse8",
            237 => "IfTrue8",
            238 => "GoTo8",
            239 => "GoTo16",
            240 => "Call0",
            241 => "Call1",
            242 => "Call2",
            243 => "Call3",
            244 => "IsUndefined",
            245 => "IsNull",
            246 => "TypeOfIsUndefined",
            247 => "TypeOfIsFunction",
            _ => "Unknown",
        }
        .to_string()
    }

    pub fn discriminant(&self) -> u8 {
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}
