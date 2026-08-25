"""Stable public GreatGramma exceptions."""

from __future__ import annotations

from typing import ClassVar, Final

from . import _native


class GreatGrammaError(RuntimeError):
    """Base error carrying a stable machine-readable code and context."""

    default_code: ClassVar[str] = "greatgramma_error"

    def __init__(
        self,
        message: str,
        *,
        code: str | None = None,
        row: int | None = None,
        token: int | None = None,
        position: int | None = None,
    ) -> None:
        super().__init__(message)
        self.code = self.default_code if code is None else code
        self.row = row
        self.token = token
        self.position = position


class CompileError(GreatGrammaError):
    default_code = "compile_error"


class TokenizerCompatibilityError(GreatGrammaError):
    default_code = "tokenizer_compatibility"


class ConfigurationError(GreatGrammaError):
    default_code = "configuration_error"


class SequenceDiscontinuityError(GreatGrammaError):
    default_code = "sequence_discontinuity"


class ConstraintViolationError(GreatGrammaError):
    default_code = "constraint_violation"


class NoValidTokenError(GreatGrammaError):
    default_code = "no_valid_token"


class SequenceCompletedError(GreatGrammaError):
    default_code = "sequence_completed"


class InternalError(GreatGrammaError):
    default_code = "internal_error"


_ERROR_TYPES: Final[dict[str, type[GreatGrammaError]]] = {
    "compile_error": CompileError,
    "tokenizer_compatibility": TokenizerCompatibilityError,
    "configuration_error": ConfigurationError,
    "sequence_discontinuity": SequenceDiscontinuityError,
    "constraint_violation": ConstraintViolationError,
    "no_valid_token": NoValidTokenError,
    "sequence_completed": SequenceCompletedError,
    "internal_error": InternalError,
}


def from_native(
    error: _native.NativeError,
    *,
    position: int | None = None,
) -> GreatGrammaError:
    """Translate the private native tuple ABI into stable public exceptions."""

    args = error.args
    if len(args) != 4:
        return InternalError(
            "native error did not follow the private error ABI",
            position=position,
        )
    code, message, row, token = args
    if not isinstance(code, str) or not isinstance(message, str):
        return InternalError(
            "native error contained invalid metadata",
            position=position,
        )
    error_type = _ERROR_TYPES.get(code)
    if error_type is None:
        error_type = InternalError
    return error_type(
        message,
        row=row if isinstance(row, int) else None,
        token=token if isinstance(token, int) else None,
        position=position,
    )
