package de.tu_darmstadt.cs.encrypto.hybridpir;

public class RustInterface {
    public native byte[] sendQuery(
        String[] targets,
        int db_size,
        int element_size,
        int raidpir_redundancy,
        int raidpir_size,
        int sealpir_degree,
        int sealpir_log,
        int sealpir_d,
        int index
    );
}
