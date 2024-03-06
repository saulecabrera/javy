file_path = './crates/quickjs-wasm-sys/quickjs/quickjs-opcode.h' # replace with your file path
File.open(file_path, 'r') do |file|
  count = 0
  file.each_line do |line|
    if line.start_with?('DEF')
      opcode_id = line.split('(')[1].split(',')[0].strip
      puts "Opcode: #{opcode_id}, Discriminant Decimal: #{count}, Hexadecimal: #{count.to_s(16)}"
      count += 1
    end
  end
end
