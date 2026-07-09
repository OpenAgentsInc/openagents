import { defineSchedule } from "eve/schedules";
import { processDueSarahFollowUps } from "../../src/services/follow-up-scheduler";

export default defineSchedule({
  cron: "* * * * *",
  async run({ waitUntil }) {
    waitUntil(processDueSarahFollowUps({ limit: 25 }));
  },
});
