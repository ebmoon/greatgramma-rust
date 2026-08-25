from __future__ import annotations

from dataclasses import dataclass
from typing import Any

import pytest

from greatgramma import ConfigurationError
from greatgramma.transformers import generate


@dataclass
class FakeGenerationConfig:
    eos_token_id: object = (3, 4)
    pad_token_id: object = 0
    num_beams: object = 1
    num_return_sequences: object = 1
    num_beam_groups: object = 1
    penalty_alpha: object = None
    prompt_lookup_num_tokens: object = None
    use_mtp: object = False
    assistant_early_exit: object = None
    token_healing: object = False
    continuous_batching: object = False
    continuous_batching_config: object = None
    constraints: object = None
    force_words_ids: object = None
    dola_layers: object = None
    do_sample: object = False


@dataclass
class FakeModelConfig:
    is_encoder_decoder: bool = False


class FakeModel:
    def __init__(self) -> None:
        self.config = FakeModelConfig()
        self.calls: list[dict[str, Any]] = []

    def generate(self, **kwargs: Any) -> str:
        self.calls.append(kwargs)
        return "generated"


class FakeCompiled:
    vocab_size = 5
    eos_token_ids = frozenset((3, 4))

    def __init__(self) -> None:
        self.processor = object()
        self.processor_calls: list[tuple[object, int | None, bool]] = []

    def logits_processor(
        self,
        prompt: object,
        *,
        pad_token_id: int | None = None,
        _trusted_fixed_append: bool = False,
    ) -> object:
        self.processor_calls.append((prompt, pad_token_id, _trusted_fixed_append))
        return self.processor


def test_generate_installs_processor_last() -> None:
    compiled = FakeCompiled()
    model = FakeModel()
    first = object()
    second = object()
    config = FakeGenerationConfig(do_sample=True)
    prompt = [[1], [2]]

    result = generate(
        compiled,
        model,
        prompt,
        generation_config=config,
        logits_processors=(first, second),
    )

    assert result == "generated"
    assert compiled.processor_calls == [(prompt, 0, True)]
    assert len(model.calls) == 1
    assert model.calls[0]["input_ids"] is prompt
    assert model.calls[0]["generation_config"] is config
    assert model.calls[0]["logits_processor"] == [first, second, compiled.processor]


@pytest.mark.parametrize(
    ("field", "value"),
    (
        ("num_beams", 2),
        ("num_return_sequences", 2),
        ("num_beam_groups", 2),
        ("penalty_alpha", 0.6),
        ("prompt_lookup_num_tokens", 4),
        ("use_mtp", True),
        ("assistant_early_exit", 3),
        ("token_healing", True),
        ("continuous_batching", True),
        ("continuous_batching_config", object()),
        ("constraints", object()),
        ("force_words_ids", [[1]]),
        ("dola_layers", "low"),
    ),
)
def test_generate_rejects_unsupported_modes_before_processor_creation(
    field: str,
    value: object,
) -> None:
    compiled = FakeCompiled()
    model = FakeModel()
    config = FakeGenerationConfig()
    setattr(config, field, value)

    with pytest.raises(ConfigurationError):
        generate(compiled, model, [[1]], generation_config=config)

    assert compiled.processor_calls == []


def test_real_generation_config_rejects_append_breaking_modes() -> None:
    transformers = pytest.importorskip("transformers", minversion="5.14")
    generation_config_type = getattr(transformers, "GenerationConfig")

    for field, value in (
        ("use_mtp", True),
        ("assistant_early_exit", 3),
        ("token_healing", True),
        ("continuous_batching_config", object()),
    ):
        config = generation_config_type(eos_token_id=[3, 4], pad_token_id=0)
        setattr(config, field, value)
        compiled = FakeCompiled()

        with pytest.raises(ConfigurationError, match=field):
            generate(compiled, FakeModel(), [[1]], generation_config=config)

        assert compiled.processor_calls == []


@pytest.mark.parametrize("do_sample", (False, True))
@pytest.mark.parametrize("model_vocab_size", (2, 4))
def test_real_transformers_generate_respects_grammar_and_eos(
    do_sample: bool,
    model_vocab_size: int,
) -> None:
    torch = pytest.importorskip("torch")
    transformers = pytest.importorskip("transformers", minversion="5.14")
    from greatgramma import Terminal, TokenizerManifest, compile as compile_grammar

    torch.manual_seed(0)
    model = transformers.GPT2LMHeadModel(
        transformers.GPT2Config(
            vocab_size=model_vocab_size,
            n_positions=8,
            n_embd=8,
            n_layer=1,
            n_head=1,
            bos_token_id=0,
            eos_token_id=1,
            pad_token_id=1,
        )
    )
    model.eval()
    compiled = compile_grammar(
        "%token ITEM\n%%\nS: ITEM;\n",
        start_rule="S",
        terminals=[Terminal("ITEM", "a")],
        tokenizer=TokenizerManifest((b"a", b""), frozenset((1,))),
    )
    config = transformers.GenerationConfig(
        max_new_tokens=2,
        do_sample=do_sample,
        eos_token_id=1,
        pad_token_id=1,
    )

    output = compiled.generate(
        model,
        torch.tensor([[0]], dtype=torch.long),
        generation_config=config,
    )

    assert output.tolist() == [[0, 0, 1]]


def test_generate_rejects_assisted_generation() -> None:
    compiled = FakeCompiled()

    with pytest.raises(ConfigurationError):
        generate(
            compiled,
            FakeModel(),
            [[1]],
            generation_config=FakeGenerationConfig(),
            assistant_model=object(),
        )

    assert compiled.processor_calls == []


def test_generate_rejects_invalid_eos_members_instead_of_silently_filtering() -> None:
    compiled = FakeCompiled()
    config = FakeGenerationConfig(eos_token_id=[3, 4, "bad"])

    with pytest.raises(ConfigurationError, match="EOS"):
        generate(compiled, FakeModel(), [[1]], generation_config=config)

    assert compiled.processor_calls == []


def test_generate_validates_callable_before_consuming_compiled_grammar() -> None:
    class MissingGenerate:
        config = FakeModelConfig()

    compiled = FakeCompiled()

    with pytest.raises(ConfigurationError, match="generate"):
        generate(
            compiled,
            MissingGenerate(),
            [[1]],
            generation_config=FakeGenerationConfig(),
        )

    assert compiled.processor_calls == []


def test_batched_generation_requires_integer_padding() -> None:
    compiled = FakeCompiled()

    with pytest.raises(ConfigurationError, match="pad"):
        generate(
            compiled,
            FakeModel(),
            [[1], [2]],
            generation_config=FakeGenerationConfig(pad_token_id=None),
        )

    assert compiled.processor_calls == []


def test_batched_generation_rejects_ambiguous_eos_padding() -> None:
    compiled = FakeCompiled()

    with pytest.raises(ConfigurationError, match="also an EOS"):
        generate(
            compiled,
            FakeModel(),
            [[1], [2]],
            generation_config=FakeGenerationConfig(pad_token_id=3),
        )

    assert compiled.processor_calls == []


def test_single_row_generation_rejects_non_integer_padding() -> None:
    compiled = FakeCompiled()

    with pytest.raises(ConfigurationError, match="pad"):
        generate(
            compiled,
            FakeModel(),
            [[1]],
            generation_config=FakeGenerationConfig(pad_token_id="0"),
        )

    assert compiled.processor_calls == []
