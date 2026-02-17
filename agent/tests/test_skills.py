"""Tests for the Skills & Plugins System"""

import pytest
import asyncio
from skills import (
    Skill,
    SkillContext,
    SkillRegistry,
    SkillLoader,
    SkillManager,
    SkillMetadata,
    SkillType,
    ToolSkill,
)
from skills.examples import (
    CalculatorSkill,
    CALCULATOR_METADATA,
    TextAnalyzerSkill,
    TEXT_ANALYZER_METADATA,
)


class TestSkillMetadata:
    """Test SkillMetadata"""
    
    def test_metadata_creation(self):
        """Test creating skill metadata"""
        metadata = SkillMetadata(
            name="test_skill",
            version="1.0.0",
            author="Test Author",
            description="A test skill",
            skill_type=SkillType.TOOL,
        )
        
        assert metadata.name == "test_skill"
        assert metadata.version == "1.0.0"
        assert metadata.skill_type == SkillType.TOOL
    
    def test_metadata_to_dict(self):
        """Test converting metadata to dict"""
        metadata = SkillMetadata(
            name="test_skill",
            version="1.0.0",
            author="Test Author",
            description="A test skill",
            skill_type=SkillType.TOOL,
            tags=["test", "sample"],
        )
        
        data = metadata.to_dict()
        assert data["name"] == "test_skill"
        assert data["skill_type"] == "tool"
        assert data["tags"] == ["test", "sample"]
    
    def test_metadata_from_dict(self):
        """Test creating metadata from dict"""
        data = {
            "name": "test_skill",
            "version": "1.0.0",
            "author": "Test Author",
            "description": "A test skill",
            "skill_type": "tool",
            "tags": ["test"],
        }
        
        metadata = SkillMetadata.from_dict(data)
        assert metadata.name == "test_skill"
        assert metadata.skill_type == SkillType.TOOL


class TestSkillContext:
    """Test SkillContext"""
    
    def test_context_creation(self):
        """Test creating a skill context"""
        context = SkillContext(
            skill_name="test_skill",
            user_id="user123",
        )
        
        assert context.skill_name == "test_skill"
        assert context.user_id == "user123"
    
    def test_permission_checking(self):
        """Test permission checking"""
        context = SkillContext(
            skill_name="test_skill",
            permissions=["read", "write"],
        )
        
        assert context.has_permission("read")
        assert context.has_permission("write")
        assert not context.has_permission("delete")
        
        # Test wildcard permission
        context2 = SkillContext(
            skill_name="test_skill",
            permissions=["*"],
        )
        assert context2.has_permission("anything")


class TestSkillRegistry:
    """Test SkillRegistry"""
    
    @pytest.mark.asyncio
    async def test_register_skill(self):
        """Test registering a skill"""
        registry = SkillRegistry()
        skill = CalculatorSkill(CALCULATOR_METADATA)
        
        registry.register(CALCULATOR_METADATA, skill)
        
        retrieved = registry.get("calculator")
        assert retrieved == skill
    
    @pytest.mark.asyncio
    async def test_list_skills(self):
        """Test listing skills"""
        registry = SkillRegistry()
        
        skill1 = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill1)
        
        skill2 = TextAnalyzerSkill(TEXT_ANALYZER_METADATA)
        registry.register(TEXT_ANALYZER_METADATA, skill2)
        
        all_skills = registry.list_skills()
        assert len(all_skills) == 2
        
        tool_skills = registry.list_skills(SkillType.TOOL)
        assert len(tool_skills) == 1
        assert tool_skills[0].name == "calculator"
    
    @pytest.mark.asyncio
    async def test_unregister_skill(self):
        """Test unregistering a skill"""
        registry = SkillRegistry()
        skill = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill)
        
        assert registry.get("calculator") is not None
        
        registry.unregister("calculator", "1.0.0")
        assert registry.get("calculator") is None


class TestCalculatorSkill:
    """Test CalculatorSkill"""
    
    @pytest.mark.asyncio
    async def test_calculator_add(self):
        """Test calculator addition"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        context = SkillContext(skill_name="calculator")
        
        await skill.initialize(context)
        
        result = await skill.execute(context, operation="add", a=5, b=3)
        assert result == 8
    
    @pytest.mark.asyncio
    async def test_calculator_subtract(self):
        """Test calculator subtraction"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        context = SkillContext(skill_name="calculator")
        
        await skill.initialize(context)
        
        result = await skill.execute(context, operation="subtract", a=10, b=4)
        assert result == 6
    
    @pytest.mark.asyncio
    async def test_calculator_multiply(self):
        """Test calculator multiplication"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        context = SkillContext(skill_name="calculator")
        
        await skill.initialize(context)
        
        result = await skill.execute(context, operation="multiply", a=6, b=7)
        assert result == 42
    
    @pytest.mark.asyncio
    async def test_calculator_divide(self):
        """Test calculator division"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        context = SkillContext(skill_name="calculator")
        
        await skill.initialize(context)
        
        result = await skill.execute(context, operation="divide", a=20, b=4)
        assert result == 5
    
    @pytest.mark.asyncio
    async def test_calculator_divide_by_zero(self):
        """Test division by zero error"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        context = SkillContext(skill_name="calculator")
        
        await skill.initialize(context)
        
        with pytest.raises(ValueError):
            await skill.execute(context, operation="divide", a=10, b=0)
    
    def test_calculator_schema(self):
        """Test calculator schema"""
        skill = CalculatorSkill(CALCULATOR_METADATA)
        schema = skill.get_schema()
        
        assert "properties" in schema
        assert "operation" in schema["properties"]
        assert "a" in schema["properties"]
        assert "b" in schema["properties"]


class TestTextAnalyzerSkill:
    """Test TextAnalyzerSkill"""
    
    @pytest.mark.asyncio
    async def test_text_wordcount(self):
        """Test word counting"""
        skill = TextAnalyzerSkill(TEXT_ANALYZER_METADATA)
        context = SkillContext(skill_name="text_analyzer")
        
        await skill.initialize(context)
        
        result = await skill.execute(
            context,
            text="Hello world from LuminaGuard",
            analysis_type="wordcount",
        )
        
        assert result["word_count"] == 4
        assert result["char_count"] > 0
    
    @pytest.mark.asyncio
    async def test_text_sentiment(self):
        """Test sentiment analysis"""
        skill = TextAnalyzerSkill(TEXT_ANALYZER_METADATA)
        context = SkillContext(skill_name="text_analyzer")
        
        await skill.initialize(context)
        
        # Positive text
        result = await skill.execute(
            context,
            text="This is great and wonderful",
            analysis_type="sentiment",
        )
        assert result["sentiment"] == "positive"
        
        # Negative text
        result = await skill.execute(
            context,
            text="This is terrible and awful",
            analysis_type="sentiment",
        )
        assert result["sentiment"] == "negative"


class TestSkillManager:
    """Test SkillManager"""
    
    @pytest.mark.asyncio
    async def test_enable_disable_skill(self):
        """Test enabling and disabling skills"""
        registry = SkillRegistry()
        loader = SkillLoader(registry)
        manager = SkillManager(registry, loader)
        
        skill = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill)
        
        # Disable skill
        await manager.disable_skill("calculator")
        assert not skill.is_enabled
        
        # Enable skill
        await manager.enable_skill("calculator")
        assert skill.is_enabled
    
    @pytest.mark.asyncio
    async def test_execute_skill(self):
        """Test executing a skill through manager"""
        registry = SkillRegistry()
        loader = SkillLoader(registry)
        manager = SkillManager(registry, loader)
        
        skill = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill)
        
        await skill.initialize(SkillContext(skill_name="calculator"))
        
        result = await manager.execute_skill(
            "calculator",
            SkillContext(skill_name="calculator"),
            operation="add",
            a=3,
            b=4,
        )
        
        assert result == 7
    
    @pytest.mark.asyncio
    async def test_execute_disabled_skill(self):
        """Test that executing a disabled skill fails"""
        registry = SkillRegistry()
        loader = SkillLoader(registry)
        manager = SkillManager(registry, loader)
        
        skill = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill)
        
        await skill.initialize(SkillContext(skill_name="calculator"))
        await manager.disable_skill("calculator")
        
        with pytest.raises(RuntimeError):
            await manager.execute_skill(
                "calculator",
                SkillContext(skill_name="calculator"),
                operation="add",
                a=1,
                b=1,
            )
    
    @pytest.mark.asyncio
    async def test_get_skill_schema(self):
        """Test getting skill schema"""
        registry = SkillRegistry()
        loader = SkillLoader(registry)
        manager = SkillManager(registry, loader)
        
        skill = CalculatorSkill(CALCULATOR_METADATA)
        registry.register(CALCULATOR_METADATA, skill)
        
        schema = await manager.get_skill_schema("calculator")
        assert schema is not None
        assert "properties" in schema


if __name__ == "__main__":
    pytest.main([__file__])
