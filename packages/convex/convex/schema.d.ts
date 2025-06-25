/**
 * Convex database schema for OpenAgents
 * @since 1.0.0
 */
declare const _default: import("convex/server").SchemaDefinition<{
    events: import("convex/server").TableDefinition<import("convex/values").VObject<{
        relay_url?: string;
        id: string;
        pubkey: string;
        created_at: number;
        tags: string[][];
        content: string;
        sig: string;
        received_at: number;
        kind: number;
    }, {
        id: import("convex/values").VString<string, "required">;
        pubkey: import("convex/values").VString<string, "required">;
        created_at: import("convex/values").VFloat64<number, "required">;
        kind: import("convex/values").VFloat64<number, "required">;
        tags: import("convex/values").VArray<string[][], import("convex/values").VArray<string[], import("convex/values").VString<string, "required">, "required">, "required">;
        content: import("convex/values").VString<string, "required">;
        sig: import("convex/values").VString<string, "required">;
        received_at: import("convex/values").VFloat64<number, "required">;
        relay_url: import("convex/values").VString<string | undefined, "optional">;
    }, "required", "id" | "pubkey" | "created_at" | "tags" | "content" | "sig" | "received_at" | "relay_url" | "kind">, {
        by_pubkey_created: ["pubkey", "created_at", "_creationTime"];
        by_kind_created: ["kind", "created_at", "_creationTime"];
        by_created_at: ["created_at", "_creationTime"];
        by_kind_pubkey: ["kind", "pubkey", "_creationTime"];
        by_received_at: ["received_at", "_creationTime"];
    }, {}, {}>;
    event_tags: import("convex/server").TableDefinition<import("convex/values").VObject<{
        event_id: string;
        tag_name: string;
        tag_value: string;
        tag_index: number;
    }, {
        event_id: import("convex/values").VString<string, "required">;
        tag_name: import("convex/values").VString<string, "required">;
        tag_value: import("convex/values").VString<string, "required">;
        tag_index: import("convex/values").VFloat64<number, "required">;
    }, "required", "event_id" | "tag_name" | "tag_value" | "tag_index">, {
        by_tag_name_value: ["tag_name", "tag_value", "_creationTime"];
        by_event_id: ["event_id", "_creationTime"];
        by_tag_name: ["tag_name", "_creationTime"];
        by_tag_value: ["tag_value", "_creationTime"];
    }, {}, {}>;
    agent_profiles: import("convex/server").TableDefinition<import("convex/values").VObject<{
        name?: string;
        balance?: number;
        metabolic_rate?: number;
        profile_event_id?: string;
        pubkey: string;
        created_at: number;
        agent_id: string;
        status: string;
        capabilities: string[];
        last_activity: number;
        updated_at: number;
    }, {
        pubkey: import("convex/values").VString<string, "required">;
        agent_id: import("convex/values").VString<string, "required">;
        name: import("convex/values").VString<string | undefined, "optional">;
        status: import("convex/values").VString<string, "required">;
        balance: import("convex/values").VFloat64<number | undefined, "optional">;
        metabolic_rate: import("convex/values").VFloat64<number | undefined, "optional">;
        capabilities: import("convex/values").VArray<string[], import("convex/values").VString<string, "required">, "required">;
        last_activity: import("convex/values").VFloat64<number, "required">;
        profile_event_id: import("convex/values").VString<string | undefined, "optional">;
        created_at: import("convex/values").VFloat64<number, "required">;
        updated_at: import("convex/values").VFloat64<number, "required">;
    }, "required", "pubkey" | "created_at" | "agent_id" | "name" | "status" | "balance" | "metabolic_rate" | "capabilities" | "last_activity" | "profile_event_id" | "updated_at">, {
        by_agent_id: ["agent_id", "_creationTime"];
        by_status: ["status", "_creationTime"];
        by_last_activity: ["last_activity", "_creationTime"];
        by_balance: ["balance", "_creationTime"];
    }, {}, {}>;
    service_offerings: import("convex/server").TableDefinition<import("convex/values").VObject<{
        offering_event_id?: string;
        id: string;
        created_at: number;
        capabilities: string[];
        updated_at: number;
        agent_pubkey: string;
        service_name: string;
        nip90_kinds: number[];
        pricing: {
            per_unit?: string | undefined;
            currency?: string | undefined;
            base: number;
        };
        availability: string;
    }, {
        id: import("convex/values").VString<string, "required">;
        agent_pubkey: import("convex/values").VString<string, "required">;
        service_name: import("convex/values").VString<string, "required">;
        nip90_kinds: import("convex/values").VArray<number[], import("convex/values").VFloat64<number, "required">, "required">;
        pricing: import("convex/values").VObject<{
            per_unit?: string | undefined;
            currency?: string | undefined;
            base: number;
        }, {
            base: import("convex/values").VFloat64<number, "required">;
            per_unit: import("convex/values").VString<string | undefined, "optional">;
            currency: import("convex/values").VString<string | undefined, "optional">;
        }, "required", "base" | "per_unit" | "currency">;
        capabilities: import("convex/values").VArray<string[], import("convex/values").VString<string, "required">, "required">;
        availability: import("convex/values").VString<string, "required">;
        offering_event_id: import("convex/values").VString<string | undefined, "optional">;
        created_at: import("convex/values").VFloat64<number, "required">;
        updated_at: import("convex/values").VFloat64<number, "required">;
    }, "required", "id" | "created_at" | "capabilities" | "updated_at" | "agent_pubkey" | "service_name" | "nip90_kinds" | "pricing" | "availability" | "offering_event_id" | "pricing.base" | "pricing.per_unit" | "pricing.currency">, {
        by_agent_pubkey: ["agent_pubkey", "_creationTime"];
        by_service_name: ["service_name", "_creationTime"];
        by_availability: ["availability", "_creationTime"];
    }, {}, {}>;
    channels: import("convex/server").TableDefinition<import("convex/values").VObject<{
        name?: string;
        about?: string;
        picture?: string;
        last_message_at?: number;
        id: string;
        created_at: number;
        updated_at: number;
        creator_pubkey: string;
        created_by: string;
        message_count: number;
    }, {
        id: import("convex/values").VString<string, "required">;
        name: import("convex/values").VString<string | undefined, "optional">;
        about: import("convex/values").VString<string | undefined, "optional">;
        picture: import("convex/values").VString<string | undefined, "optional">;
        creator_pubkey: import("convex/values").VString<string, "required">;
        created_by: import("convex/values").VString<string, "required">;
        message_count: import("convex/values").VFloat64<number, "required">;
        last_message_at: import("convex/values").VFloat64<number | undefined, "optional">;
        created_at: import("convex/values").VFloat64<number, "required">;
        updated_at: import("convex/values").VFloat64<number, "required">;
    }, "required", "id" | "created_at" | "name" | "updated_at" | "about" | "picture" | "creator_pubkey" | "created_by" | "message_count" | "last_message_at">, {
        by_name: ["name", "_creationTime"];
        by_creator: ["creator_pubkey", "_creationTime"];
        by_last_message: ["last_message_at", "_creationTime"];
        by_message_count: ["message_count", "_creationTime"];
    }, {}, {}>;
    job_requests: import("convex/server").TableDefinition<import("convex/values").VObject<{
        request_event_id?: string;
        provider_pubkey?: string;
        result_data?: any;
        id: string;
        created_at: number;
        status: string;
        updated_at: number;
        requester_pubkey: string;
        service_type: string;
        description: string;
        payment_amount: number;
    }, {
        id: import("convex/values").VString<string, "required">;
        request_event_id: import("convex/values").VString<string | undefined, "optional">;
        requester_pubkey: import("convex/values").VString<string, "required">;
        provider_pubkey: import("convex/values").VString<string | undefined, "optional">;
        service_type: import("convex/values").VString<string, "required">;
        status: import("convex/values").VString<string, "required">;
        description: import("convex/values").VString<string, "required">;
        payment_amount: import("convex/values").VFloat64<number, "required">;
        result_data: import("convex/values").VAny<any, "optional", string>;
        created_at: import("convex/values").VFloat64<number, "required">;
        updated_at: import("convex/values").VFloat64<number, "required">;
    }, "required", "id" | "created_at" | "status" | "updated_at" | "request_event_id" | "requester_pubkey" | "provider_pubkey" | "service_type" | "description" | "payment_amount" | "result_data" | `result_data.${string}`>, {
        by_requester: ["requester_pubkey", "_creationTime"];
        by_provider: ["provider_pubkey", "_creationTime"];
        by_status: ["status", "_creationTime"];
        by_service_type: ["service_type", "_creationTime"];
        by_created_at: ["created_at", "_creationTime"];
    }, {}, {}>;
    relay_stats: import("convex/server").TableDefinition<import("convex/values").VObject<{
        metric_name: string;
        metric_value: number;
        timestamp: number;
        metadata: any;
    }, {
        metric_name: import("convex/values").VString<string, "required">;
        metric_value: import("convex/values").VFloat64<number, "required">;
        timestamp: import("convex/values").VFloat64<number, "required">;
        metadata: import("convex/values").VAny<any, "required", string>;
    }, "required", "metric_name" | "metric_value" | "timestamp" | "metadata" | `metadata.${string}`>, {
        by_metric_timestamp: ["metric_name", "timestamp", "_creationTime"];
        by_timestamp: ["timestamp", "_creationTime"];
    }, {}, {}>;
    sessions: import("convex/server").TableDefinition<import("convex/values").VObject<{
        project_name?: string;
        id: string;
        status: string;
        last_activity: number;
        message_count: number;
        user_id: string;
        project_path: string;
        started_at: number;
        total_cost: number;
    }, {
        id: import("convex/values").VString<string, "required">;
        user_id: import("convex/values").VString<string, "required">;
        project_path: import("convex/values").VString<string, "required">;
        project_name: import("convex/values").VString<string | undefined, "optional">;
        status: import("convex/values").VString<string, "required">;
        started_at: import("convex/values").VFloat64<number, "required">;
        last_activity: import("convex/values").VFloat64<number, "required">;
        message_count: import("convex/values").VFloat64<number, "required">;
        total_cost: import("convex/values").VFloat64<number, "required">;
    }, "required", "id" | "status" | "last_activity" | "message_count" | "user_id" | "project_path" | "project_name" | "started_at" | "total_cost">, {
        by_user_id: ["user_id", "_creationTime"];
        by_status: ["status", "_creationTime"];
        by_last_activity: ["last_activity", "_creationTime"];
        by_project_path: ["project_path", "_creationTime"];
    }, {}, {}>;
    messages: import("convex/server").TableDefinition<import("convex/values").VObject<{
        content?: string;
        role?: string;
        thinking?: string;
        summary?: string;
        model?: string;
        token_usage?: {
            input_tokens: number;
            output_tokens: number;
            total_tokens: number;
        };
        cost?: number;
        turn_count?: number;
        tool_name?: string;
        tool_input?: any;
        tool_use_id?: string;
        tool_output?: string;
        tool_is_error?: boolean;
        embedding_id?: import("convex/values").GenericId<"message_embeddings">;
        timestamp: number;
        session_id: string;
        entry_uuid: string;
        entry_type: string;
    }, {
        session_id: import("convex/values").VString<string, "required">;
        entry_uuid: import("convex/values").VString<string, "required">;
        entry_type: import("convex/values").VString<string, "required">;
        role: import("convex/values").VString<string | undefined, "optional">;
        content: import("convex/values").VString<string | undefined, "optional">;
        thinking: import("convex/values").VString<string | undefined, "optional">;
        summary: import("convex/values").VString<string | undefined, "optional">;
        model: import("convex/values").VString<string | undefined, "optional">;
        token_usage: import("convex/values").VObject<{
            input_tokens: number;
            output_tokens: number;
            total_tokens: number;
        } | undefined, {
            input_tokens: import("convex/values").VFloat64<number, "required">;
            output_tokens: import("convex/values").VFloat64<number, "required">;
            total_tokens: import("convex/values").VFloat64<number, "required">;
        }, "optional", "input_tokens" | "output_tokens" | "total_tokens">;
        cost: import("convex/values").VFloat64<number | undefined, "optional">;
        timestamp: import("convex/values").VFloat64<number, "required">;
        turn_count: import("convex/values").VFloat64<number | undefined, "optional">;
        tool_name: import("convex/values").VString<string | undefined, "optional">;
        tool_input: import("convex/values").VAny<any, "optional", string>;
        tool_use_id: import("convex/values").VString<string | undefined, "optional">;
        tool_output: import("convex/values").VString<string | undefined, "optional">;
        tool_is_error: import("convex/values").VBoolean<boolean | undefined, "optional">;
        embedding_id: import("convex/values").VId<import("convex/values").GenericId<"message_embeddings"> | undefined, "optional">;
    }, "required", "content" | "timestamp" | "session_id" | "entry_uuid" | "entry_type" | "role" | "thinking" | "summary" | "model" | "token_usage" | "cost" | "turn_count" | "tool_name" | "tool_input" | "tool_use_id" | "tool_output" | "tool_is_error" | "embedding_id" | "token_usage.input_tokens" | "token_usage.output_tokens" | "token_usage.total_tokens" | `tool_input.${string}`>, {
        by_session_id: ["session_id", "_creationTime"];
        by_entry_type: ["entry_type", "_creationTime"];
        by_timestamp: ["timestamp", "_creationTime"];
        by_tool_use_id: ["tool_use_id", "_creationTime"];
        by_embedding_id: ["embedding_id", "_creationTime"];
    }, {}, {}>;
    message_embeddings: import("convex/server").TableDefinition<import("convex/values").VObject<{
        created_at: number;
        model: string;
        message_id: import("convex/values").GenericId<"messages">;
        embedding: number[];
        dimensions: number;
    }, {
        message_id: import("convex/values").VId<import("convex/values").GenericId<"messages">, "required">;
        embedding: import("convex/values").VArray<number[], import("convex/values").VFloat64<number, "required">, "required">;
        model: import("convex/values").VString<string, "required">;
        dimensions: import("convex/values").VFloat64<number, "required">;
        created_at: import("convex/values").VFloat64<number, "required">;
    }, "required", "created_at" | "model" | "message_id" | "embedding" | "dimensions">, {
        by_message_id: ["message_id", "_creationTime"];
    }, {}, {
        by_embedding: {
            vectorField: "embedding";
            dimensions: number;
            filterFields: "model";
        };
    }>;
    images: import("convex/server").TableDefinition<import("convex/values").VObject<{
        message_id: import("convex/values").GenericId<"messages">;
        image_data: string;
        mime_type: string;
        position: number;
    }, {
        message_id: import("convex/values").VId<import("convex/values").GenericId<"messages">, "required">;
        image_data: import("convex/values").VString<string, "required">;
        mime_type: import("convex/values").VString<string, "required">;
        position: import("convex/values").VFloat64<number, "required">;
    }, "required", "message_id" | "image_data" | "mime_type" | "position">, {
        by_message_id: ["message_id", "_creationTime"];
        by_position: ["position", "_creationTime"];
    }, {}, {}>;
}, true>;
export default _default;
//# sourceMappingURL=schema.d.ts.map