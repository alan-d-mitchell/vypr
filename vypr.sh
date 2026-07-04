vyprc_file="vyprc.txt"

> "$vyprc_file"

find . -type f -name "*.rs" | while read -r file; do
echo "$file" >> "$vyprc_file"
cat "$file" >> "$vyprc_file"
echo -e "\n" >> "$vyprc_file"
done
