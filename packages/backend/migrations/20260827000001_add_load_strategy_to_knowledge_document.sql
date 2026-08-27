-- 项目知识文档增加加载策略字段
-- eager: harness 启动时全量加载（默认）
-- lazy: 按需渐进式披露，harness 启动时不加载，由 Agent 决策后单条拉取
ALTER TABLE project_knowledge_document
ADD COLUMN load_strategy VARCHAR(16) NOT NULL DEFAULT 'eager';

-- 约束：只允许 eager / lazy 两个值
ALTER TABLE project_knowledge_document
ADD CONSTRAINT project_knowledge_document_load_strategy_check
CHECK (load_strategy IN ('eager', 'lazy'));

-- 索引：按项目+策略过滤时加速
CREATE INDEX idx_project_knowledge_document_project_strategy
ON project_knowledge_document (project_id, load_strategy);
