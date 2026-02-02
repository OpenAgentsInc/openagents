interface FormattedDateProps {
  date: Date | string;
}

export default function FormattedDate({ date }: FormattedDateProps) {
  const parsed = typeof date === "string" ? new Date(date) : date;
  const value = Number.isNaN(parsed.valueOf()) ? new Date(0) : parsed;

  return (
    <time dateTime={value.toISOString()}>
      {value.toLocaleDateString("en-us", {
        year: "numeric",
        month: "short",
        day: "numeric",
        timeZone: "UTC",
      })}
    </time>
  );
}
