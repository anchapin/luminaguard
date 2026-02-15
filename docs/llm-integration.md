# LLM Integration for LuminaGuard Agent

## Overview

This document describes the LLM integration layer implemented for the LuminaGuard agent reasoning loop. The integration replaces the placeholder keyword-based reasoning with a flexible LLM-based decision-making system.

## Architecture

### Components

1. **llm_client.py** - LLM client abstraction layer
   - `LLMClient` - Abstract base class for all LLM clients
   - `MockLLMClient` - Mock client for testing (deterministic)
   - `OpenAILLMClient` - Production client for GPT models
   - `create_llm_client()` - Factory function for client creation

2. **loop.py** - Updated reasoning loop
   - `think()` - Now uses LLM for decision-making
   - `run_loop()` - Supports multi-turn reasoning

### Key Features

#### 1. Multi-Turn Reasoning

The agent can execute multiple tools in sequence as it works through complex tasks:

```python
# First iteration: User requests action
state = AgentState(
    messages=[{"role": "user", "content": "List files then read test.txt"}],
    tools=["list_directory", "read_file"],
    context={}
)
action = think(state, llm_client)
# Returns: ToolCall(name="list_directory", ...)

# Second iteration: After tool execution
state.add_message("tool", "File list result...")
action = think(state, llm_client)
# Returns: ToolCall(name="read_file", ...)

# Third iteration: After all tools complete
action = think(state, llm_client)
# Returns: None (task complete)
```

#### 2. Context-Based Tool Selection

Tools are selected based on conversation context, not simple keywords:

```python
# The LLM analyzes the full message history
messages = [
    {"role": "user", "content": "I want to analyze the logs"},
    {"role": "assistant", "content": "I'll help you. Which log file?"},
    {"role": "user", "content": "The one in /var/log/system.log"},
]
# LLM understands context and selects appropriate tool
```

#### 3. Deterministic Behavior

Default configuration uses `temperature=0` for reproducible results:

```python
config = LLMConfig(
    provider=LLMProvider.OPENAI,
    model="gpt-4",
    temperature=0.0,  # Deterministic
)
```

## Usage

### Basic Usage (Mock Client)

```python
from loop import AgentState, think
from llm_client import MockLLMClient

# Create mock LLM client
llm_client = MockLLMClient()

# Run reasoning
state = AgentState(
    messages=[{"role": "user", "content": "Read the file"}],
    tools=["read_file"],
    context={}
)
action = think(state, llm_client)

if action:
    print(f"Tool: {action.name}")
    print(f"Arguments: {action.arguments}")
    print(f"Action Kind: {action.action_kind}")
```

### Production Usage (OpenAI)

```python
from loop import AgentState, think, run_loop
from llm_client import LLMConfig, LLMProvider, create_llm_client

# Create OpenAI client
config = LLMConfig(
    provider=LLMProvider.OPENAI,
    model="gpt-4",
    api_key=os.getenv("OPENAI_API_KEY"),
    temperature=0.0,
)
llm_client = create_llm_client(config)

# Run full reasoning loop
state = run_loop(
    task="Read /tmp/config.yaml and check settings",
    tools=["read_file", "search"],
    llm_client=llm_client,
)
```

## Security

### Action Classification

Actions are classified as GREEN (autonomous) or RED (requires approval) based on tool names:

```python
GREEN = ["read", "list", "search", "check", "get", "show", "view", ...]
RED = ["delete", "remove", "write", "edit", "modify", "create", ...]
```

The LLM decides which tool to use, but the security layer classifies the action kind.

### Deterministic Output

Production use should always set `temperature=0` to ensure:
- Reproducible results
- Predictable behavior
- Consistent testing

## Testing

### Unit Tests

```bash
cd agent
python -m pytest tests/test_llm_integration.py -v
```

### Coverage

The LLM integration has 77% coverage:

- `llm_client.py`: 77% (covers MockLLMClient, configuration)
- `loop.py`: 64% (covers think(), run_loop())
- `tests/test_llm_integration.py`: 662 lines, 72 tests

### Test Categories

1. **Configuration Tests**
   - Default config values
   - Custom config values
   - Temperature settings

2. **MockLLMClient Tests**
   - Tool selection logic
   - Multi-turn reasoning
   - Edge cases

3. **Integration Tests**
   - `think()` function with LLM
   - `run_loop()` with multi-turn
   - Action kind determination

4. **Property-Based Tests**
   - Hypothesis tests for various inputs
   - Robustness validation

## Design Decisions

### Why MockLLMClient?

1. **Deterministic Testing**: Mock client provides predictable outputs
2. **No API Costs**: Doesn't require LLM API keys during testing
3. **Fast Execution**: No network calls, tests run in milliseconds
4. **Privacy**: Test data never leaves the machine

### Why Lazy Import?

LLM client is imported inside `think()` to avoid circular dependencies:

```python
def think(state, llm_client=None):
    try:
        from llm_client import MockLLMClient, LLMConfig
        pass  # use LLM
    except ImportError:
        pass  # Fallback to simple implementation
```

This allows the module to work even if `llm_client.py` is not available.

### Why Abstract Base Class?

The `LLMClient` ABC allows:
- Easy addition of new LLM providers (Anthropic, local models, etc.)
- Consistent interface across all providers
- Swappable implementations for testing vs production

## Future Enhancements

### Planned Features

1. **Anthropic Claude Support**

```python
from llm_client import AnthropicLLMClient
client = AnthropicLLMClient(config)
```

2. **Local LLM Support (Ollama)**

```python
from llm_client import OllamaLLMClient
client = OllamaLLMClient(config)
```

3. **Tool Definitions**
   - Automatic tool schema generation
   - Dynamic tool discovery from MCP

4. **Conversation Memory**
   - Persistent conversation state
   - Context window management

5. **Streaming Responses**
   - Real-time token streaming
   - Early stopping for efficiency

## Performance

### Benchmarks

- MockLLMClient: ~0.1ms per decision (no network)
- OpenAI GPT-4: ~1-2s per decision (network dependent)

### Optimization Tips

1. Use MockLLMClient for development/testing
2. Cache LLM responses when appropriate
3. Batch multiple tool requests
4. Use smaller models (GPT-3.5) for simple tasks

## Troubleshooting

### Import Errors

```python
# Error: ModuleNotFoundError: No module named 'openai'
# Solution: pip install openai
```

### API Key Errors

```python
# Error: openai.OpenAIError: The api_key client option must be set
# Solution: Set OPENAI_API_KEY environment variable or pass in config
```

### Timeout Errors

```python
# Error: Timeout waiting for LLM response
# Solution: Increase timeout in config
config = LLMConfig(timeout=60)  # 60 seconds
```

## API Reference

### LLMConfig

```python
@dataclass
class LLMConfig:
    provider: LLMProvider = LLMProvider.MOCK
    model: str = "mock-model"
    api_key: Optional[str] = None
    base_url: Optional[str] = None
    temperature: float = 0.0
    max_tokens: int = 1000
    timeout: int = 30
```

### LLMClient (ABC)

```python
class LLMClient(ABC):
    @abstractmethod
    def decide_action(
        self,
        messages: List[Dict[str, Any]],
        available_tools: List[str],
        context: Dict[str, Any],
    ) -> LLMResponse:
        """Decide next action based on state."""
        pass
```

### LLMResponse

```python
class LLMResponse:
    def __init__(
        self,
        tool_name: Optional[str],
        arguments: Dict[str, Any],
        reasoning: str,
        is_complete: bool,
    ):
        self.tool_name = tool_name
        self.arguments = arguments
        self.reasoning = reasoning
        self.is_complete = is_complete
```

## References

- Issue #193: Replace placeholder keyword-based reasoning with LLM integration
- CLAUDE.md: Project architecture guidelines
- loop.py: Main agent reasoning loop
- tests/test_llm_integration.py: Comprehensive test suite
