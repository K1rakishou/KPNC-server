ALTER TABLE post_replies
ADD COLUMN notification_delivery_attempt INT2 DEFAULT 0,
ADD COLUMN notification_delivered_on TIMESTAMP WITH TIME ZONE DEFAULT NULL;

UPDATE post_replies
SET notification_delivered_on = NOW()
WHERE post_replies.id_generated IN (
    SELECT post_replies.id_generated
    FROM post_replies
    WHERE post_replies.notification_sent_on IS NOT NULL
)