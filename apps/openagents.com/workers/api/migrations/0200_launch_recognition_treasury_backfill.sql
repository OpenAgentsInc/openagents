-- Backfill public-safe launch-recognition attribution for the 2026-06-16/17
-- treasury payout incident. This stores only recipient refs, owed refs, and
-- public-safe confirmation refs; it does not add destinations, invoices,
-- payment hashes, preimages, mnemonics, or wallet material.

UPDATE treasury_transactions
SET recipient_ref = 'recipient.public.launch_recognition.trigger',
    owed_ref = 'owed.public.launch_recognition.trigger.2026-06-16',
    owed_sat = 50000,
    recipient_confirmation_state = 'confirmed_received',
    recipient_confirmation_ref = 'recipient_confirmation.public.launch_recognition.trigger.spark_backup_status_2026-06-17',
    recipient_confirmed_at = '2026-06-17T01:53:48.180Z'
WHERE id = 'treasury_payout_3734c1a9-b5a4-4551-a956-98eb0bad5a7f';

UPDATE treasury_transactions
SET recipient_ref = 'recipient.public.launch_recognition.whitefang',
    owed_ref = 'owed.public.launch_recognition.whitefang.2026-06-16',
    owed_sat = 50000,
    recipient_confirmation_state = 'confirmed_received',
    recipient_confirmation_ref = 'recipient_confirmation.public.launch_recognition.whitefang.spark_backup_canary_2026-06-16',
    recipient_confirmed_at = '2026-06-16T22:06:51.768Z'
WHERE id = 'treasury_payout_ec113054-6aed-4dba-a936-0d537191d74b';

UPDATE treasury_transactions
SET recipient_ref = 'recipient.public.launch_recognition.orrery',
    owed_ref = 'owed.public.launch_recognition.orrery.2026-06-16',
    owed_sat = 50000
WHERE id IN (
  'tips_buffer_payout_acf6dd7c-26e3-450a-b431-c7c32f944dc9',
  'treasury_payout_daad3604-6646-4b98-abef-c6a79619f830',
  'treasury_payout_23c170b1-51c7-4dbc-b2d4-ced722dab619',
  'treasury_payout_3280b034-ad67-4645-9cb0-f02b87624832',
  'treasury_payout_72537490-d8fd-45d2-b832-78065802a7e4',
  'treasury_payout_a53042f8-3f4c-46d6-8734-a4067f7755d1',
  'treasury_payout_8a446b77-3b06-42b8-9a6a-b21a3ad8b68c',
  'treasury_payout_32e21012-7946-418f-ba69-af29fd733b4f',
  'treasury_payout_84040137-7195-4c7b-aeb3-56182829ea37',
  'treasury_payout_d97b64a6-6884-4e28-bad9-455a9b143447',
  'treasury_payout_36693707-0114-4445-b9fe-b90e2bb837a8',
  'treasury_payout_ff7d7445-feca-4954-9808-2aae003b116f',
  'tips_buffer_payout_0914dc67-0c16-4470-9308-0cae7fbc274d',
  'treasury_payout_b9e1440a-0858-48ab-8c85-2145167fed33',
  'treasury_payout_ecadabc5-a7f0-4899-ac15-2ea87bbd6ec9',
  'tips_buffer_payout_e1c888be-01a8-4bb4-ad6f-e576771841ad',
  'treasury_payout_37c811c4-d3b5-4159-b2e8-25e45d0fcacf',
  'treasury_payout_8e8affb4-0cc2-42bd-a713-2424abd072ea',
  'treasury_payout_08b07d03-8ee6-4476-82b1-fb8fe1586ca8',
  'treasury_payout_dc67bf74-4014-482e-8902-eb5498f7acb7',
  'treasury_payout_2cbe9074-9ffa-427d-8a4a-8b14bd4c5cbe',
  'treasury_payout_5c293488-aa85-4c62-ab6a-ebc0292def17',
  'treasury_payout_2f32693e-4fc2-4c51-97ef-1acb027c00c0',
  'treasury_payout_f0665059-9d9a-40bb-ba44-e081ca1e8721',
  'tips_buffer_payout_5c7f8b80-972e-47d5-af66-aefe3db82672',
  'tips_buffer_payout_da542562-7271-444c-b38e-579c3a5e10da',
  'tips_buffer_payout_418ecb0b-9b60-469e-9ced-9ff1ac14d530',
  'tips_buffer_payout_03c59ff0-37bc-404e-9a28-de77282b3a06',
  'tips_buffer_payout_33ed7e04-5be3-406c-8a5e-925180f82010'
);
