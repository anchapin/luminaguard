"""
Example skills for LuminaGuard Skills System

Demonstrates how to create custom skills.
"""

from typing import Any, Dict
import logging
from . import Skill, SkillMetadata, SkillContext, SkillType

logger = logging.getLogger(__name__)


# Example 1: Simple calculator tool skill
CALCULATOR_METADATA = SkillMetadata(
    name="calculator",
    version="1.0.0",
    author="LuminaGuard Team",
    description="Simple calculator skill for basic math operations",
    skill_type=SkillType.TOOL,
    tags=["math", "calculator"],
    requires_approval=False,
)


class CalculatorSkill(Skill):
    """A simple calculator skill"""
    
    async def initialize(self, context: SkillContext) -> bool:
        logger.info("Calculator skill initialized")
        return True
    
    async def execute(self, context: SkillContext, **kwargs) -> Any:
        """
        Execute calculator operations
        
        Supported kwargs:
        - operation: "add", "subtract", "multiply", "divide"
        - a: first number
        - b: second number
        """
        operation = kwargs.get("operation", "add")
        a = kwargs.get("a", 0)
        b = kwargs.get("b", 0)
        
        if operation == "add":
            return a + b
        elif operation == "subtract":
            return a - b
        elif operation == "multiply":
            return a * b
        elif operation == "divide":
            if b == 0:
                raise ValueError("Division by zero")
            return a / b
        else:
            raise ValueError(f"Unknown operation: {operation}")
    
    async def cleanup(self) -> None:
        pass
    
    def get_schema(self) -> Dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "subtract", "multiply", "divide"],
                },
                "a": {"type": "number"},
                "b": {"type": "number"},
            },
            "required": ["operation", "a", "b"],
        }


# Example 2: Web scraping skill
WEB_SCRAPER_METADATA = SkillMetadata(
    name="web_scraper",
    version="1.0.0",
    author="LuminaGuard Team",
    description="Skill for scraping web content",
    skill_type=SkillType.INTEGRATION,
    tags=["web", "scraping"],
    requires_approval=True,  # Requires approval due to network access
    allowed_permissions=["network.http", "storage.write"],
)


class WebScraperSkill(Skill):
    """Web scraping skill"""
    
    async def initialize(self, context: SkillContext) -> bool:
        if not context.has_permission("network.http"):
            logger.error("Web scraper requires network.http permission")
            return False
        logger.info("Web scraper skill initialized")
        return True
    
    async def execute(self, context: SkillContext, **kwargs) -> Any:
        """
        Scrape web content
        
        Kwargs:
        - url: URL to scrape
        - selector: CSS selector (optional)
        """
        url = kwargs.get("url")
        if not url:
            raise ValueError("URL is required")
        
        try:
            import aiohttp
            
            async with aiohttp.ClientSession() as session:
                async with session.get(url, timeout=10) as resp:
                    if resp.status == 200:
                        return await resp.text()
                    else:
                        raise RuntimeError(f"HTTP {resp.status}")
        except Exception as e:
            logger.error(f"Web scraping error: {e}")
            raise
    
    async def cleanup(self) -> None:
        pass
    
    def get_schema(self) -> Dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "url": {"type": "string", "format": "uri"},
                "selector": {"type": "string"},
            },
            "required": ["url"],
        }


# Example 3: Text analysis skill
TEXT_ANALYZER_METADATA = SkillMetadata(
    name="text_analyzer",
    version="1.0.0",
    author="LuminaGuard Team",
    description="Skill for analyzing text content",
    skill_type=SkillType.ANALYZER,
    tags=["text", "analysis", "nlp"],
    requires_approval=False,
)


class TextAnalyzerSkill(Skill):
    """Text analysis skill"""
    
    async def initialize(self, context: SkillContext) -> bool:
        logger.info("Text analyzer skill initialized")
        return True
    
    async def execute(self, context: SkillContext, **kwargs) -> Any:
        """
        Analyze text
        
        Kwargs:
        - text: Text to analyze
        - analysis_type: "wordcount", "sentiment", "entities"
        """
        text = kwargs.get("text", "")
        analysis_type = kwargs.get("analysis_type", "wordcount")
        
        if analysis_type == "wordcount":
            return {
                "word_count": len(text.split()),
                "char_count": len(text),
                "line_count": len(text.split("\n")),
            }
        elif analysis_type == "sentiment":
            # Simple sentiment analysis (would use real NLP lib in practice)
            negative_words = {"bad", "terrible", "awful", "horrible"}
            positive_words = {"good", "great", "excellent", "wonderful"}
            
            words = text.lower().split()
            neg_count = sum(1 for w in words if w in negative_words)
            pos_count = sum(1 for w in words if w in positive_words)
            
            return {
                "positive_count": pos_count,
                "negative_count": neg_count,
                "sentiment": "positive" if pos_count > neg_count else "negative" if neg_count > pos_count else "neutral",
            }
        else:
            raise ValueError(f"Unknown analysis type: {analysis_type}")
    
    async def cleanup(self) -> None:
        pass
    
    def get_schema(self) -> Dict[str, Any]:
        return {
            "type": "object",
            "properties": {
                "text": {"type": "string"},
                "analysis_type": {
                    "type": "string",
                    "enum": ["wordcount", "sentiment", "entities"],
                },
            },
            "required": ["text"],
        }
