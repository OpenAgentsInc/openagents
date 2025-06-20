CREATE TABLE `agent_profiles` (
	`pubkey` varchar(64) NOT NULL,
	`agent_id` varchar(255) NOT NULL,
	`name` varchar(255),
	`status` varchar(32) NOT NULL DEFAULT 'active',
	`balance` bigint DEFAULT 0,
	`metabolic_rate` bigint DEFAULT 100,
	`capabilities` json DEFAULT ('[]'),
	`last_activity` timestamp DEFAULT (now()),
	`profile_event_id` varchar(64),
	`created_at` timestamp NOT NULL DEFAULT (now()),
	`updated_at` timestamp NOT NULL DEFAULT (now()) ON UPDATE CURRENT_TIMESTAMP,
	CONSTRAINT `agent_profiles_pubkey` PRIMARY KEY(`pubkey`)
);
--> statement-breakpoint
CREATE TABLE `channels` (
	`id` varchar(64) NOT NULL,
	`name` varchar(255),
	`about` text,
	`picture` varchar(500),
	`creator_pubkey` varchar(64) NOT NULL,
	`message_count` bigint DEFAULT 0,
	`last_message_at` timestamp,
	`created_at` timestamp NOT NULL DEFAULT (now()),
	`updated_at` timestamp NOT NULL DEFAULT (now()) ON UPDATE CURRENT_TIMESTAMP,
	CONSTRAINT `channels_id` PRIMARY KEY(`id`)
);
--> statement-breakpoint
CREATE TABLE `event_tags` (
	`id` bigint AUTO_INCREMENT NOT NULL,
	`event_id` varchar(64) NOT NULL,
	`tag_name` varchar(64) NOT NULL,
	`tag_value` varchar(255) NOT NULL,
	`tag_index` bigint NOT NULL,
	CONSTRAINT `event_tags_id` PRIMARY KEY(`id`)
);
--> statement-breakpoint
CREATE TABLE `events` (
	`id` varchar(64) NOT NULL,
	`pubkey` varchar(64) NOT NULL,
	`created_at` bigint NOT NULL,
	`kind` int NOT NULL,
	`tags` json NOT NULL,
	`content` text NOT NULL,
	`sig` varchar(128) NOT NULL,
	`received_at` timestamp NOT NULL DEFAULT (now()),
	`relay_url` varchar(255) DEFAULT 'ws://localhost:3000/relay',
	CONSTRAINT `events_id` PRIMARY KEY(`id`)
);
--> statement-breakpoint
CREATE TABLE `relay_stats` (
	`id` bigint AUTO_INCREMENT NOT NULL,
	`metric_name` varchar(64) NOT NULL,
	`metric_value` bigint NOT NULL,
	`timestamp` timestamp NOT NULL DEFAULT (now()),
	`metadata` json DEFAULT ('{}'),
	CONSTRAINT `relay_stats_id` PRIMARY KEY(`id`)
);
--> statement-breakpoint
CREATE TABLE `service_offerings` (
	`id` varchar(255) NOT NULL,
	`agent_pubkey` varchar(64) NOT NULL,
	`service_name` varchar(255) NOT NULL,
	`nip90_kinds` json NOT NULL,
	`pricing` json NOT NULL,
	`capabilities` json DEFAULT ('[]'),
	`availability` varchar(32) DEFAULT 'available',
	`offering_event_id` varchar(64),
	`created_at` timestamp NOT NULL DEFAULT (now()),
	`updated_at` timestamp NOT NULL DEFAULT (now()) ON UPDATE CURRENT_TIMESTAMP,
	CONSTRAINT `service_offerings_id` PRIMARY KEY(`id`)
);
--> statement-breakpoint
ALTER TABLE `agent_profiles` ADD CONSTRAINT `agent_profiles_profile_event_id_events_id_fk` FOREIGN KEY (`profile_event_id`) REFERENCES `events`(`id`) ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE `event_tags` ADD CONSTRAINT `event_tags_event_id_events_id_fk` FOREIGN KEY (`event_id`) REFERENCES `events`(`id`) ON DELETE cascade ON UPDATE no action;--> statement-breakpoint
ALTER TABLE `service_offerings` ADD CONSTRAINT `service_offerings_agent_pubkey_agent_profiles_pubkey_fk` FOREIGN KEY (`agent_pubkey`) REFERENCES `agent_profiles`(`pubkey`) ON DELETE no action ON UPDATE no action;--> statement-breakpoint
ALTER TABLE `service_offerings` ADD CONSTRAINT `service_offerings_offering_event_id_events_id_fk` FOREIGN KEY (`offering_event_id`) REFERENCES `events`(`id`) ON DELETE no action ON UPDATE no action;--> statement-breakpoint
CREATE INDEX `idx_agent_id` ON `agent_profiles` (`agent_id`);--> statement-breakpoint
CREATE INDEX `idx_status` ON `agent_profiles` (`status`);--> statement-breakpoint
CREATE INDEX `idx_last_activity` ON `agent_profiles` (`last_activity`);--> statement-breakpoint
CREATE INDEX `idx_balance` ON `agent_profiles` (`balance`);--> statement-breakpoint
CREATE INDEX `idx_name` ON `channels` (`name`);--> statement-breakpoint
CREATE INDEX `idx_creator` ON `channels` (`creator_pubkey`);--> statement-breakpoint
CREATE INDEX `idx_last_message` ON `channels` (`last_message_at`);--> statement-breakpoint
CREATE INDEX `idx_message_count` ON `channels` (`message_count`);--> statement-breakpoint
CREATE INDEX `idx_tag_name_value` ON `event_tags` (`tag_name`,`tag_value`);--> statement-breakpoint
CREATE INDEX `idx_event_id` ON `event_tags` (`event_id`);--> statement-breakpoint
CREATE INDEX `idx_tag_name` ON `event_tags` (`tag_name`);--> statement-breakpoint
CREATE INDEX `idx_tag_value` ON `event_tags` (`tag_value`);--> statement-breakpoint
CREATE INDEX `idx_pubkey_created` ON `events` (`pubkey`,`created_at`);--> statement-breakpoint
CREATE INDEX `idx_kind_created` ON `events` (`kind`,`created_at`);--> statement-breakpoint
CREATE INDEX `idx_created_at` ON `events` (`created_at`);--> statement-breakpoint
CREATE INDEX `idx_kind_pubkey` ON `events` (`kind`,`pubkey`);--> statement-breakpoint
CREATE INDEX `idx_received_at` ON `events` (`received_at`);--> statement-breakpoint
CREATE INDEX `idx_metric_timestamp` ON `relay_stats` (`metric_name`,`timestamp`);--> statement-breakpoint
CREATE INDEX `idx_timestamp` ON `relay_stats` (`timestamp`);--> statement-breakpoint
CREATE INDEX `idx_agent_pubkey` ON `service_offerings` (`agent_pubkey`);--> statement-breakpoint
CREATE INDEX `idx_service_name` ON `service_offerings` (`service_name`);--> statement-breakpoint
CREATE INDEX `idx_availability` ON `service_offerings` (`availability`);